#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "wasm32", no_std)]
extern crate alloc;
use alloc::boxed::Box;
use alloc::{vec, vec::Vec};
// Multi-merchant prepaid subscription accounting for AVM SDK 0.3.0.
use avm_contract_sdk::{
    AccountAddress, CallbackIntent, Caller, CanonicalMessage, Contract, CurrencyAmount,
    CurrencyAmounts, EventField, EventIntent, EventKind, EventValue, MessageInput, MessageOutput,
    OutboundIntent, StateUpdate,
};
pub mod ids;
pub const PRINCIPAL: u32 = 0;
pub const SHELL: u32 = 2;
pub const MAX_WALLETS: usize = 8;
pub const MAX_MERCHANTS: usize = 4;
pub const MAX_PLANS: usize = 8;
pub const MAX_SUBS: usize = 16;
pub const MAX_ATTEMPTS: usize = 128;
pub const MAX_PERIODS: u32 = 24;
pub const STATE_BYTES: usize = 12288;
pub const CONFIG_BYTES: usize = 122;
pub const ARGS: u32 = 4000;
pub const AUTH: u32 = 4001;
pub const STATE: u32 = 4002;
pub const PHASE: u32 = 4003;
pub const VALUE: u32 = 4004;
pub const TIME: u32 = 4005;
pub const PENDING: u32 = 4006;
pub const CALLBACK: u32 = 4007;
pub const LIMIT: u32 = 4008;
pub const BALANCE: u32 = 4009;
pub const PAUSED: u32 = 4010;
pub const STALE: u32 = 4011;
pub type Result<T> = core::result::Result<T, u32>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub admin: [u8; 32],
    pub treasury: AccountAddress,
    pub assets: [u32; 2],
    pub fee_bps: u16,
    pub fuel: u128,
}
impl Config {
    #[inline(never)]
    pub fn validate(&self) -> Result<()> {
        if self.admin == [0; 32]
            || !valid_address(self.treasury)
            || self.assets[0] >= self.assets[1]
            || self.assets.contains(&SHELL)
            || self.fee_bps > 1000
            || self.fuel == 0
        {
            Err(ARGS)
        } else {
            Ok(())
        }
    }
    #[inline(never)]
    pub fn encode(&self) -> Vec<u8> {
        let mut b = self.admin.to_vec();
        put_address(&mut b, self.treasury);
        for a in self.assets {
            put(&mut b, a.to_le_bytes());
        }
        put(&mut b, self.fee_bps.to_le_bytes());
        put(&mut b, self.fuel.to_le_bytes());
        b
    }
    #[inline(never)]
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() != CONFIG_BYTES {
            return Err(ARGS);
        }
        let mut r = Reader::new(b);
        let s = Self {
            admin: r.array()?,
            treasury: r.address()?,
            assets: [r.u32()?, r.u32()?],
            fee_bps: r.u16()?,
            fuel: r.u128()?,
        };
        s.validate()?;
        Ok(s)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wallet {
    pub owner: [u8; 32],
    pub destination: AccountAddress,
    pub credit: [u128; 2],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub merchant: u8,
    pub asset: u8,
    pub version: u32,
    pub price: u128,
    pub period: u32,
    pub active: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    pub amount: u128,
    pub until: u32,
    pub digest: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription {
    pub id: u64,
    pub wallet: u8,
    pub plan: u8,
    pub merchant: u8,
    pub asset: u8,
    pub price: u128,
    pub period: u32,
    pub version: u32,
    pub start: u32,
    pub terms: u32,
    pub billed: u128,
    pub last_time: u32,
    pub offer_nonce: u32,
    pub offer: Option<Offer>,
}
impl Subscription {
    pub fn budget(&self) -> Result<u128> {
        self.price.checked_mul(u128::from(self.terms)).ok_or(LIMIT)
    }
    pub fn reserve(&self) -> Result<u128> {
        self.budget()?.checked_sub(self.billed).ok_or(STATE)
    }
    pub fn end(&self) -> Result<u32> {
        self.period
            .checked_mul(self.terms)
            .and_then(|n| self.start.checked_add(n))
            .ok_or(TIME)
    }
    pub fn due(&self, now: u32) -> Result<u128> {
        if now < self.start || now < self.last_time {
            return Err(TIME);
        }
        let elapsed = now.min(self.end()?) - self.start;
        mul_div(self.price, u128::from(elapsed), u128::from(self.period))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub kind: u8,
    pub owner: u8,
    pub asset: u8,
    pub attempt: u64,
    pub amount: u128,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub config: Config,
    pub deposited: [u128; 2],
    pub withdrawn: [u128; 2],
    pub fees: [u128; 2],
    pub next_id: u64,
    pub attempts: u64,
    pub paused: bool,
    pub proposed_admin: [u8; 32],
    pub wallets: Vec<Wallet>,
    pub merchants: Vec<Wallet>,
    pub plans: Vec<Plan>,
    pub subs: Vec<Subscription>,
    pub pending: Option<Pending>,
    pub receipts: Vec<[u8; 32]>,
}
impl State {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            deposited: [0; 2],
            withdrawn: [0; 2],
            fees: [0; 2],
            next_id: 1,
            attempts: 0,
            paused: false,
            proposed_admin: [0; 32],
            wallets: vec![],
            merchants: vec![],
            plans: vec![],
            subs: vec![],
            pending: None,
            receipts: vec![],
        }
    }
    #[inline(never)]
    pub fn liabilities(&self) -> Result<[u128; 2]> {
        let mut a = self.fees;
        for w in self.wallets.iter().chain(&self.merchants) {
            for (i, v) in a.iter_mut().enumerate() {
                *v = v.checked_add(w.credit[i]).ok_or(STATE)?;
            }
        }
        for s in &self.subs {
            let i = s.asset as usize;
            if i > 1 {
                return Err(STATE);
            }
            a[i] = a[i].checked_add(s.reserve()?).ok_or(STATE)?;
        }
        Ok(a)
    }
    #[inline(never)]
    pub fn validate(&self) -> Result<()> {
        self.config.validate().map_err(|_| STATE)?;
        if self.wallets.len() > MAX_WALLETS
            || self.merchants.len() > MAX_MERCHANTS
            || self.plans.len() > MAX_PLANS
            || self.subs.len() > MAX_SUBS
            || self.next_id == 0
            || self.attempts > MAX_ATTEMPTS as u64
            || self.receipts.len() + usize::from(self.pending.is_some()) != self.attempts as usize
            || self.proposed_admin == self.config.admin
        {
            return Err(STATE);
        }
        for list in [&self.wallets, &self.merchants] {
            for (i, w) in list.iter().enumerate() {
                if w.owner == [0; 32]
                    || !valid_address(w.destination)
                    || list[..i].iter().any(|x| x.owner == w.owner)
                {
                    return Err(STATE);
                }
            }
        }
        for p in &self.plans {
            if p.merchant as usize >= self.merchants.len()
                || p.asset > 1
                || p.price == 0
                || p.period == 0
                || p.version == 0
            {
                return Err(STATE);
            }
        }
        for (i, s) in self.subs.iter().enumerate() {
            let p = self.plans.get(s.plan as usize).ok_or(STATE)?;
            if s.id == 0
                || s.id >= self.next_id
                || self.subs[..i].iter().any(|x| x.id == s.id)
                || s.wallet as usize >= self.wallets.len()
                || s.merchant != p.merchant
                || s.asset != p.asset
                || s.version == 0
                || s.version > p.version
                || s.price == 0
                || s.period == 0
                || s.terms == 0
                || s.terms > MAX_PERIODS
                || s.last_time < s.start
                || s.billed > s.budget()?
                || s.billed > s.due(s.last_time)?
            {
                return Err(STATE);
            }
            s.end()?;
            if let Some(o) = &s.offer {
                if o.amount > s.reserve()?
                    || o.digest == [0; 32]
                    || o.until <= s.last_time
                    || s.offer_nonce == 0
                {
                    return Err(STATE);
                }
            }
        }
        if self.receipts.contains(&[0; 32]) || self.receipts.windows(2).any(|w| w[0] >= w[1]) {
            return Err(STATE);
        }
        let liabilities = self.liabilities()?;
        for (i, n) in liabilities.iter().enumerate() {
            if self.deposited[i].checked_sub(self.withdrawn[i]) != Some(*n) {
                return Err(STATE);
            }
        }
        if let Some(p) = self.pending {
            if p.asset > 1
                || p.amount == 0
                || p.attempt != self.attempts
                || self.credit(p.kind, p.owner, p.asset)? < p.amount
            {
                return Err(STATE);
            }
        }
        Ok(())
    }
    pub fn credit(&self, kind: u8, owner: u8, asset: u8) -> Result<u128> {
        let a = asset as usize;
        if a > 1 {
            return Err(ARGS);
        }
        match kind {
            1 => Ok(self.wallets.get(owner as usize).ok_or(ARGS)?.credit[a]),
            2 => Ok(self.merchants.get(owner as usize).ok_or(ARGS)?.credit[a]),
            3 if owner == 0 => Ok(self.fees[a]),
            _ => Err(ARGS),
        }
    }
    pub fn destination(&self, p: Pending) -> Result<AccountAddress> {
        match p.kind {
            1 => Ok(self.wallets.get(p.owner as usize).ok_or(STATE)?.destination),
            2 => Ok(self
                .merchants
                .get(p.owner as usize)
                .ok_or(STATE)?
                .destination),
            3 if p.owner == 0 => Ok(self.config.treasury),
            _ => Err(STATE),
        }
    }
    pub fn owner_key(&self, p: Pending) -> Result<[u8; 32]> {
        match p.kind {
            1 => Ok(self.wallets.get(p.owner as usize).ok_or(STATE)?.owner),
            2 => Ok(self.merchants.get(p.owner as usize).ok_or(STATE)?.owner),
            3 if p.owner == 0 => Ok(self.config.admin),
            _ => Err(STATE),
        }
    }
    #[inline(never)]
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::from(*b"ASH1");
        b.extend(self.config.encode());
        for list in [self.deposited, self.withdrawn, self.fees] {
            for x in list {
                put(&mut b, x.to_le_bytes());
            }
        }
        put(&mut b, self.next_id.to_le_bytes());
        put(&mut b, self.attempts.to_le_bytes());
        b.push(self.paused as u8);
        b.extend(self.proposed_admin);
        b.extend([
            self.wallets.len() as u8,
            self.merchants.len() as u8,
            self.plans.len() as u8,
            self.subs.len() as u8,
            self.receipts.len() as u8,
        ]);
        if let Some(p) = self.pending {
            b.extend([p.kind, p.owner, p.asset]);
            put(&mut b, p.attempt.to_le_bytes());
            put(&mut b, p.amount.to_le_bytes());
        } else {
            b.extend([0; 27]);
        }
        for w in self.wallets.iter().chain(&self.merchants) {
            b.extend(w.owner);
            put_address(&mut b, w.destination);
            for n in w.credit {
                put(&mut b, n.to_le_bytes());
            }
        }
        for p in &self.plans {
            b.extend([p.merchant, p.asset]);
            put(&mut b, p.version.to_le_bytes());
            put(&mut b, p.price.to_le_bytes());
            put(&mut b, p.period.to_le_bytes());
            b.push(p.active as u8);
        }
        for s in &self.subs {
            put(&mut b, s.id.to_le_bytes());
            b.extend([s.wallet, s.plan, s.merchant, s.asset]);
            put(&mut b, s.price.to_le_bytes());
            for n in [s.period, s.version, s.start, s.terms] {
                put(&mut b, n.to_le_bytes());
            }
            put(&mut b, s.billed.to_le_bytes());
            put(&mut b, s.last_time.to_le_bytes());
            put(&mut b, s.offer_nonce.to_le_bytes());
            b.push(s.offer.is_some() as u8);
            if let Some(o) = &s.offer {
                put(&mut b, o.amount.to_le_bytes());
                put(&mut b, o.until.to_le_bytes());
                b.extend(o.digest);
            } else {
                b.extend([0; 52]);
            }
        }
        for h in &self.receipts {
            b.extend(h);
        }
        assert!(b.len() <= STATE_BYTES);
        b.resize(STATE_BYTES, 0);
        b
    }
    #[inline(never)]
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() != STATE_BYTES || &b[..4] != b"ASH1" {
            return Err(STATE);
        }
        let mut r = Reader::new(&b[4..]);
        let config = Config::decode(r.take(CONFIG_BYTES)?)?;
        let mut lists = [[0; 2]; 3];
        for l in &mut lists {
            for v in l {
                *v = r.u128()?;
            }
        }
        let next_id = r.u64()?;
        let attempts = r.u64()?;
        let paused = r.boolean()?;
        let proposed_admin = r.array()?;
        let counts = r.array::<5>()?;
        if counts[0] as usize > MAX_WALLETS
            || counts[1] as usize > MAX_MERCHANTS
            || counts[2] as usize > MAX_PLANS
            || counts[3] as usize > MAX_SUBS
            || counts[4] as usize > MAX_ATTEMPTS
        {
            return Err(STATE);
        }
        let kind = r.u8()?;
        let owner = r.u8()?;
        let asset = r.u8()?;
        let attempt = r.u64()?;
        let amount = r.u128()?;
        let pending = if kind == 0 {
            if owner != 0 || asset != 0 || attempt != 0 || amount != 0 {
                return Err(STATE);
            }
            None
        } else {
            Some(Pending {
                kind,
                owner,
                asset,
                attempt,
                amount,
            })
        };
        let mut wallets = vec![];
        let mut merchants = vec![];
        for (n, list) in [(counts[0], &mut wallets), (counts[1], &mut merchants)] {
            for _ in 0..n {
                list.push(Wallet {
                    owner: r.array()?,
                    destination: r.address()?,
                    credit: [r.u128()?, r.u128()?],
                });
            }
        }
        let mut plans = vec![];
        for _ in 0..counts[2] {
            plans.push(Plan {
                merchant: r.u8()?,
                asset: r.u8()?,
                version: r.u32()?,
                price: r.u128()?,
                period: r.u32()?,
                active: r.boolean()?,
            });
        }
        let mut subs = vec![];
        for _ in 0..counts[3] {
            let id = r.u64()?;
            let wallet = r.u8()?;
            let plan = r.u8()?;
            let merchant = r.u8()?;
            let asset = r.u8()?;
            let price = r.u128()?;
            let period = r.u32()?;
            let version = r.u32()?;
            let start = r.u32()?;
            let terms = r.u32()?;
            let billed = r.u128()?;
            let last_time = r.u32()?;
            let offer_nonce = r.u32()?;
            let yes = r.boolean()?;
            let amount = r.u128()?;
            let until = r.u32()?;
            let digest = r.array()?;
            let offer = if yes {
                Some(Offer {
                    amount,
                    until,
                    digest,
                })
            } else {
                if amount != 0 || until != 0 || digest != [0; 32] {
                    return Err(STATE);
                }
                None
            };
            subs.push(Subscription {
                id,
                wallet,
                plan,
                merchant,
                asset,
                price,
                period,
                version,
                start,
                terms,
                billed,
                last_time,
                offer_nonce,
                offer,
            });
        }
        let mut receipts = vec![];
        for _ in 0..counts[4] {
            receipts.push(r.array()?);
        }
        if r.take(r.remaining())?.iter().any(|x| *x != 0) {
            return Err(STATE);
        }
        let s = Self {
            config,
            deposited: lists[0],
            withdrawn: lists[1],
            fees: lists[2],
            next_id,
            attempts,
            paused,
            proposed_admin,
            wallets,
            merchants,
            plans,
            subs,
            pending,
            receipts,
        };
        s.validate()?;
        Ok(s)
    }
    fn sub_index(&self, id: u64) -> Result<usize> {
        self.subs.iter().position(|s| s.id == id).ok_or(STALE)
    }
    #[inline(never)]
    fn charge(&mut self, i: usize, target: u128, now: u32) -> Result<()> {
        let s = &mut self.subs[i];
        if now < s.last_time || target < s.billed || target > s.budget()? {
            return Err(TIME);
        }
        let delta = target - s.billed;
        let fee = mul_div(target, self.config.fee_bps.into(), 10000)?
            .checked_sub(mul_div(s.billed, self.config.fee_bps.into(), 10000)?)
            .ok_or(STATE)?;
        let a = s.asset as usize;
        let m = s.merchant as usize;
        self.merchants[m].credit[a] = self.merchants[m].credit[a]
            .checked_add(delta - fee)
            .ok_or(STATE)?;
        self.fees[a] = self.fees[a].checked_add(fee).ok_or(STATE)?;
        s.billed = target;
        s.last_time = now;
        s.offer = None;
        Ok(())
    }
    #[inline(never)]
    fn close(&mut self, i: usize) -> Result<()> {
        let s = self.subs.remove(i);
        let a = s.asset as usize;
        let w = s.wallet as usize;
        self.wallets[w].credit[a] = self.wallets[w].credit[a]
            .checked_add(s.reserve()?)
            .ok_or(STATE)?;
        Ok(())
    }
}
pub struct Hub;
impl Contract for Hub {
    fn init(i: MessageInput<'_>) -> MessageOutput {
        initialize(i).unwrap_or_else(MessageOutput::failure)
    }
    fn process_message(i: MessageInput<'_>) -> MessageOutput {
        process(i).unwrap_or_else(MessageOutput::failure)
    }
}
#[inline(never)]
fn initialize(i: MessageInput<'_>) -> Result<MessageOutput> {
    let c = Config::decode(i.deploy().map_err(|_| ARGS)?.init_data())?;
    if i.caller != Caller::External(c.admin) {
        return Err(AUTH);
    }
    if !i.state.is_empty()
        || c.treasury == i.destination
        || c.treasury.dapp_id != i.destination.dapp_id
    {
        return Err(ARGS);
    }
    zero(&i)?;
    finish(Box::new(State::new(c)), 0, 0)
}
#[inline(never)]
fn process(i: MessageInput<'_>) -> Result<MessageOutput> {
    let mut s = Box::new(State::decode(i.state)?);
    if i.message.is_callback() {
        return callback(i, s);
    }
    let action = apply(&i, &mut s)?;
    let intent = if action.kind == 11 {
        let p = s.pending.ok_or(STATE)?;
        Some(OutboundIntent {
            input_index: 0,
            destination: s.destination(p)?,
            fuel_fee_shells: s.config.fuel,
            extra: vec![CurrencyAmount {
                currency_id: s.config.assets[p.asset as usize],
                amount: p.amount,
            }],
            method_id: ids::RECEIVE,
            body: payment_body(&s)?,
        })
    } else {
        None
    };
    let mut out = finish(s, action.kind, action.id)?;
    if let Some(intent) = intent {
        out.outbound_intents.push(intent);
        out.callback_intents.push(CallbackIntent {
            outbound_index: 0,
            handler_method_id: ids::CALLBACK,
        });
    }
    Ok(out)
}
struct Action {
    kind: u64,
    id: u64,
}
#[inline(never)]
fn apply(i: &MessageInput<'_>, s: &mut State) -> Result<Action> {
    let Caller::External(actor) = i.caller else {
        return Err(AUTH);
    };
    let m = i.message.method_id();
    let body = i.message.body();
    let now = i.context.block_time;
    if m == ids::STATUS {
        empty(body)?;
        zero(i)?;
        return Ok(Action { kind: 1, id: 0 });
    }
    if m == ids::TOPUP {
        empty(body)?;
        if i.message.extra().is_empty()
            || i.message
                .extra()
                .iter()
                .any(|v| v.currency_id != SHELL || v.amount == 0)
        {
            return Err(VALUE);
        }
        return Ok(Action { kind: 2, id: 0 });
    }
    if m != ids::DEPOSIT {
        zero(i)?;
    }
    if m == ids::PAUSE {
        admin(s, actor)?;
        let mut r = Reader::new(body);
        s.paused = r.boolean()?;
        r.finish()?;
        return Ok(Action { kind: 3, id: 0 });
    }
    if s.pending.is_some() {
        return Err(PENDING);
    }
    if m == ids::ADMIN_PROPOSE {
        admin(s, actor)?;
        let mut r = Reader::new(body);
        let key = r.array()?;
        r.finish()?;
        if key == [0; 32] || key == s.config.admin {
            return Err(ARGS);
        }
        s.proposed_admin = key;
        return Ok(Action { kind: 4, id: 0 });
    }
    if m == ids::ADMIN_ACCEPT {
        empty(body)?;
        if actor != s.proposed_admin || actor == [0; 32] {
            return Err(AUTH);
        }
        s.config.admin = actor;
        s.proposed_admin = [0; 32];
        return Ok(Action { kind: 5, id: 0 });
    }
    if m == ids::WALLET_OPEN || m == ids::MERCHANT_ADD {
        let mut r = Reader::new(body);
        let owner = if m == ids::MERCHANT_ADD {
            admin(s, actor)?;
            r.array()?
        } else {
            actor
        };
        let destination = r.address()?;
        r.finish()?;
        if owner == [0; 32]
            || !valid_address(destination)
            || destination == i.destination
            || destination.dapp_id != i.destination.dapp_id
        {
            return Err(ARGS);
        }
        let (list, cap) = if m == ids::WALLET_OPEN {
            (&mut s.wallets, MAX_WALLETS)
        } else {
            (&mut s.merchants, MAX_MERCHANTS)
        };
        if list.len() == cap {
            return Err(LIMIT);
        }
        if list.iter().any(|w| w.owner == owner) {
            return Err(PHASE);
        }
        list.push(Wallet {
            owner,
            destination,
            credit: [0; 2],
        });
        return Ok(Action { kind: 6, id: 0 });
    }
    if m == ids::PLAN_NEW {
        if s.paused {
            return Err(PAUSED);
        }
        let merchant = merchant(s, actor)?;
        let mut r = Reader::new(body);
        let asset = r.u8()?;
        let price = r.u128()?;
        let period = r.u32()?;
        r.finish()?;
        if asset > 1 || price == 0 || period == 0 {
            return Err(ARGS);
        }
        if s.plans.len() == MAX_PLANS {
            return Err(LIMIT);
        }
        s.plans.push(Plan {
            merchant: merchant as u8,
            asset,
            price,
            period,
            version: 1,
            active: true,
        });
        return Ok(Action { kind: 7, id: 0 });
    }
    if m == ids::PLAN_EDIT {
        let merchant = merchant(s, actor)?;
        let mut r = Reader::new(body);
        let index = r.u8()? as usize;
        let version = r.u32()?;
        let price = r.u128()?;
        let period = r.u32()?;
        let active = r.boolean()?;
        r.finish()?;
        let p = s.plans.get_mut(index).ok_or(ARGS)?;
        if p.merchant as usize != merchant {
            return Err(AUTH);
        }
        if version != p.version {
            return Err(STALE);
        }
        if price == 0 || period == 0 {
            return Err(ARGS);
        }
        p.version = p.version.checked_add(1).ok_or(LIMIT)?;
        p.price = price;
        p.period = period;
        p.active = active;
        return Ok(Action { kind: 8, id: 0 });
    }
    if m == ids::DEPOSIT {
        if s.attempts == MAX_ATTEMPTS as u64 {
            return Err(LIMIT);
        }
        let w = wallet(s, actor)?;
        let mut r = Reader::new(body);
        let a = r.u8()? as usize;
        r.finish()?;
        if a > 1 {
            return Err(ARGS);
        }
        let amount = currency(i.message.extra(), s.config.assets[a]);
        if amount == 0 || i.message.extra().len() != 1 {
            return Err(VALUE);
        }
        s.deposited[a] = s.deposited[a].checked_add(amount).ok_or(LIMIT)?;
        s.wallets[w].credit[a] = s.wallets[w].credit[a].checked_add(amount).ok_or(LIMIT)?;
        if currency(i.extra, s.config.assets[a]) < s.deposited[a] - s.withdrawn[a] {
            return Err(BALANCE);
        }
        return Ok(Action { kind: 9, id: 0 });
    }
    if m == ids::JOIN {
        if s.paused {
            return Err(PAUSED);
        }
        if s.attempts == MAX_ATTEMPTS as u64 || s.subs.len() == MAX_SUBS {
            return Err(LIMIT);
        }
        let w = wallet(s, actor)?;
        let mut r = Reader::new(body);
        let plan = r.u8()?;
        let version = r.u32()?;
        let terms = r.u8()? as u32;
        r.finish()?;
        let p = s.plans.get(plan as usize).ok_or(ARGS)?.clone();
        if version != p.version {
            return Err(STALE);
        }
        if !p.active {
            return Err(PHASE);
        }
        if terms == 0 || terms > MAX_PERIODS {
            return Err(ARGS);
        }
        let budget = p.price.checked_mul(terms.into()).ok_or(LIMIT)?;
        let a = p.asset as usize;
        if s.wallets[w].credit[a] < budget {
            return Err(BALANCE);
        }
        let id = s.next_id;
        s.next_id = s.next_id.checked_add(1).ok_or(LIMIT)?;
        let sub = Subscription {
            id,
            wallet: w as u8,
            plan,
            merchant: p.merchant,
            asset: p.asset,
            price: p.price,
            period: p.period,
            version: p.version,
            start: now,
            terms,
            billed: 0,
            last_time: now,
            offer_nonce: 0,
            offer: None,
        };
        sub.end()?;
        s.wallets[w].credit[a] -= budget;
        s.subs.push(sub);
        return Ok(Action { kind: 10, id });
    }
    if m == ids::WITHDRAW_CUSTOMER || m == ids::WITHDRAW_MERCHANT || m == ids::WITHDRAW_FEES {
        let mut r = Reader::new(body);
        let asset = r.u8()?;
        let amount = r.u128()?;
        r.finish()?;
        let (kind, owner) = if m == ids::WITHDRAW_CUSTOMER {
            (1, wallet(s, actor)? as u8)
        } else if m == ids::WITHDRAW_MERCHANT {
            if s.paused {
                return Err(PAUSED);
            }
            (2, merchant(s, actor)? as u8)
        } else {
            admin(s, actor)?;
            if s.paused {
                return Err(PAUSED);
            }
            (3, 0)
        };
        if asset > 1 || amount == 0 {
            return Err(ARGS);
        }
        if amount > s.credit(kind, owner, asset)? {
            return Err(BALANCE);
        }
        if s.attempts == MAX_ATTEMPTS as u64 {
            return Err(LIMIT);
        }
        for a in 0..2 {
            if currency(i.extra, s.config.assets[a]) < s.deposited[a] - s.withdrawn[a] {
                return Err(BALANCE);
            }
        }
        if currency(i.extra, SHELL) < s.config.fuel {
            return Err(BALANCE);
        }
        s.attempts += 1;
        let p = Pending {
            kind,
            owner,
            asset,
            attempt: s.attempts,
            amount,
        };
        s.pending = Some(p);
        return Ok(Action { kind: 11, id: 0 });
    }
    let mut r = Reader::new(body);
    let id = r.u64()?;
    let n = s.sub_index(id)?;
    if m == ids::COLLECT {
        let max = r.u8()?;
        r.finish()?;
        if !(1..=4).contains(&max) {
            return Err(ARGS);
        }
        let sub = &s.subs[n];
        if now < sub.last_time {
            return Err(TIME);
        }
        let due = ((now.min(sub.end()?) - sub.start) / sub.period).min(sub.terms);
        let billed = u32::try_from(sub.billed / sub.price).map_err(|_| STATE)?;
        let periods = due.min(billed.checked_add(max.into()).ok_or(LIMIT)?);
        let target = sub.price.checked_mul(periods.into()).ok_or(LIMIT)?;
        if target <= sub.billed {
            return Err(TIME);
        }
        s.charge(n, target, now)?;
        if s.subs[n].reserve()? == 0 {
            s.close(n)?;
        }
        return Ok(Action { kind: 12, id });
    }
    if m == ids::OFFER_ACCEPT {
        let nonce = r.u32()?;
        let digest = r.array::<32>()?;
        r.finish()?;
        let sub = &s.subs[n];
        if actor != s.merchants[sub.merchant as usize].owner {
            return Err(AUTH);
        }
        let offer = sub.offer.clone().ok_or(PHASE)?;
        if nonce != sub.offer_nonce || digest != offer.digest {
            return Err(STALE);
        }
        if now > offer.until || now < sub.last_time {
            return Err(TIME);
        }
        let target = sub.billed.checked_add(offer.amount).ok_or(LIMIT)?;
        s.charge(n, target, now)?;
        s.close(n)?;
        return Ok(Action { kind: 13, id });
    }
    if actor != s.wallets[s.subs[n].wallet as usize].owner {
        return Err(AUTH);
    }
    if m == ids::EXTEND {
        if s.paused {
            return Err(PAUSED);
        }
        if s.attempts == MAX_ATTEMPTS as u64 {
            return Err(LIMIT);
        }
        let more = r.u8()? as u32;
        r.finish()?;
        if more == 0 {
            return Err(ARGS);
        }
        let sub = &mut s.subs[n];
        if now < sub.last_time || now >= sub.end()? {
            return Err(TIME);
        }
        let terms = sub.terms.checked_add(more).ok_or(LIMIT)?;
        if terms > MAX_PERIODS {
            return Err(LIMIT);
        }
        let extra = sub.price.checked_mul(more.into()).ok_or(LIMIT)?;
        let w = sub.wallet as usize;
        let a = sub.asset as usize;
        if s.wallets[w].credit[a] < extra {
            return Err(BALANCE);
        }
        sub.terms = terms;
        sub.end()?;
        sub.offer = None;
        s.wallets[w].credit[a] -= extra;
        return Ok(Action { kind: 14, id });
    }
    if m == ids::CANCEL {
        r.finish()?;
        let due = s.subs[n].due(now)?;
        s.charge(n, due, now)?;
        s.close(n)?;
        return Ok(Action { kind: 15, id });
    }
    if m == ids::OFFER_REVOKE {
        r.finish()?;
        if s.subs[n].offer.take().is_none() {
            return Err(PHASE);
        }
        return Ok(Action { kind: 16, id });
    }
    if m == ids::OFFER {
        let award = r.u128()?;
        let until = r.u32()?;
        let digest = r.array()?;
        r.finish()?;
        if digest == [0; 32] || until <= now || until > now.checked_add(86400).ok_or(TIME)? {
            return Err(ARGS);
        }
        let due = s.subs[n].due(now)?;
        s.charge(n, due, now)?;
        let sub = &mut s.subs[n];
        if award > sub.reserve()? {
            return Err(BALANCE);
        }
        sub.offer_nonce = sub.offer_nonce.checked_add(1).ok_or(LIMIT)?;
        sub.offer = Some(Offer {
            amount: award,
            until,
            digest,
        });
        return Ok(Action { kind: 17, id });
    }
    Err(ARGS)
}
#[inline(never)]
fn callback(i: MessageInput<'_>, mut s: Box<State>) -> Result<MessageOutput> {
    let (Caller::Internal(source), CanonicalMessage::Internal(msg)) = (i.caller, i.message) else {
        return Err(CALLBACK);
    };
    let p = s.pending.ok_or(CALLBACK)?;
    let cb = msg.callback.ok_or(CALLBACK)?;
    let asset = s.config.assets[p.asset as usize];
    if source != s.destination(p)?
        || msg.method_id != ids::CALLBACK
        || cb.cause_address != i.destination
        || cb.cause_msg_hash == [0; 32]
        || s.receipts.binary_search(&cb.cause_msg_hash).is_ok()
        || msg.has_callback_request()
        || msg
            .extra
            .iter()
            .any(|v| v.currency_id != SHELL && v.currency_id != asset)
    {
        return Err(CALLBACK);
    }
    let success = cb.status_code == 0;
    if success {
        if msg.body != payment_body(&s)? || msg.extra.iter().any(|v| v.currency_id != SHELL) {
            return Err(CALLBACK);
        }
        let a = p.asset as usize;
        let credit = match p.kind {
            1 => &mut s.wallets[p.owner as usize].credit[a],
            2 => &mut s.merchants[p.owner as usize].credit[a],
            3 => &mut s.fees[a],
            _ => return Err(STATE),
        };
        *credit = credit.checked_sub(p.amount).ok_or(STATE)?;
        s.withdrawn[a] = s.withdrawn[a].checked_add(p.amount).ok_or(STATE)?;
    } else if !msg.body.is_empty() || currency(msg.extra, asset) != p.amount {
        return Err(CALLBACK);
    }
    let position = s
        .receipts
        .binary_search(&cb.cause_msg_hash)
        .err()
        .ok_or(CALLBACK)?;
    s.receipts.insert(position, cb.cause_msg_hash);
    s.pending = None;
    finish(s, if success { 18 } else { 19 }, 0)
}
pub fn payment_body(s: &State) -> Result<Vec<u8>> {
    let p = s.pending.ok_or(PENDING)?;
    let mut b = Vec::from(*b"SUBP");
    put(&mut b, p.attempt.to_le_bytes());
    b.extend([p.kind, p.asset]);
    put(&mut b, s.config.assets[p.asset as usize].to_le_bytes());
    b.extend([p.owner, 0]);
    put(&mut b, p.amount.to_le_bytes());
    b.extend(s.owner_key(p)?);
    b.resize(80, 0);
    Ok(b)
}
#[inline(never)]
fn finish(s: Box<State>, kind: u64, subscription: u64) -> Result<MessageOutput> {
    s.validate()?;
    let l = s.liabilities()?;
    let fields = avm_contract_sdk::encode_event_fields(&[
        EventField {
            name: "asset0_liability".into(),
            value: EventValue::U128(l[0]),
        },
        EventField {
            name: "asset1_liability".into(),
            value: EventValue::U128(l[1]),
        },
        EventField {
            name: "kind".into(),
            value: EventValue::U64(kind),
        },
        EventField {
            name: "subscription".into(),
            value: EventValue::U64(subscription),
        },
    ])
    .map_err(|_| STATE)?;
    let mut o = MessageOutput::success(StateUpdate::FullReplace(s.encode()));
    o.event_intents.push(EventIntent {
        input_index: 0,
        kind: EventKind::External,
        event_class: ids::EVENT,
        event_name: "subscriptions.changed".into(),
        topics: vec![],
        fields_binary: fields,
    });
    Ok(o)
}
fn admin(s: &State, k: [u8; 32]) -> Result<()> {
    if k == s.config.admin {
        Ok(())
    } else {
        Err(AUTH)
    }
}
fn wallet(s: &State, k: [u8; 32]) -> Result<usize> {
    s.wallets.iter().position(|w| w.owner == k).ok_or(AUTH)
}
fn merchant(s: &State, k: [u8; 32]) -> Result<usize> {
    s.merchants.iter().position(|w| w.owner == k).ok_or(AUTH)
}
fn zero(i: &MessageInput<'_>) -> Result<()> {
    if i.message.extra().is_empty() {
        Ok(())
    } else {
        Err(VALUE)
    }
}
fn empty(b: &[u8]) -> Result<()> {
    if b.is_empty() {
        Ok(())
    } else {
        Err(ARGS)
    }
}
#[inline(never)]
pub fn mul_div(value: u128, n: u128, d: u128) -> Result<u128> {
    if d == 0 {
        return Err(ARGS);
    }
    let a = (value / d).checked_mul(n).ok_or(LIMIT)?;
    let b = (value % d).checked_mul(n).ok_or(LIMIT)? / d;
    a.checked_add(b).ok_or(LIMIT)
}
pub fn currency(c: CurrencyAmounts<'_>, id: u32) -> u128 {
    c.iter()
        .find(|x| x.currency_id == id)
        .map_or(0, |x| x.amount)
}
pub fn valid_address(a: AccountAddress) -> bool {
    a.dapp_id != [0; 32] && a.account_id != [0; 32]
}
#[inline(never)]
pub fn put_address(b: &mut Vec<u8>, a: AccountAddress) {
    b.extend(a.dapp_id);
    b.extend(a.account_id);
}
#[inline(never)]
fn put<const N: usize>(b: &mut Vec<u8>, a: [u8; N]) {
    b.extend(a);
}
pub struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> Reader<'a> {
    #[inline(never)]
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }
    #[inline(never)]
    pub fn remaining(&self) -> usize {
        self.b.len() - self.p
    }
    #[inline(never)]
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let e = self.p.checked_add(n).ok_or(ARGS)?;
        let b = self.b.get(self.p..e).ok_or(ARGS)?;
        self.p = e;
        Ok(b)
    }
    #[inline(never)]
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| ARGS)
    }
    #[inline(never)]
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    #[inline(never)]
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    #[inline(never)]
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    #[inline(never)]
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    #[inline(never)]
    pub fn u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.array()?))
    }
    #[inline(never)]
    pub fn boolean(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ARGS),
        }
    }
    #[inline(never)]
    pub fn address(&mut self) -> Result<AccountAddress> {
        Ok(AccountAddress {
            dapp_id: self.array()?,
            account_id: self.array()?,
        })
    }
    #[inline(never)]
    pub fn finish(&self) -> Result<()> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(ARGS)
        }
    }
}
