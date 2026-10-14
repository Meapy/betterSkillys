//! Condition effects (`ConditionEffect.as`): bit `n - 1` of the first batch for effects 1..31,
//! bit `n - 32` of the second batch for effects 32 and up.

pub const SLOWED: u32 = 4;
pub const CONFUSED: u32 = 11;
pub const PARALYZED: u32 = 14;
pub const SPEEDY: u32 = 15;
pub const PAUSED: u32 = 21;
pub const NINJA_SPEEDY: u32 = 29;
pub const PETRIFIED: u32 = 35;

pub fn has(conditions: [u32; 2], effect: u32) -> bool {
    if effect < 32 {
        conditions[0] & (1 << (effect - 1)) != 0
    } else {
        conditions[1] & (1 << (effect - 32)) != 0
    }
}
