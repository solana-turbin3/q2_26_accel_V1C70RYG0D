use pinocchio::error::ProgramError;

pub const TARGET_NOT_MET: u32 = 6000;
pub const TARGET_MET: u32 = 6001;
pub const CONTRIBUTION_TOO_BIG: u32 = 6002;
pub const CONTRIBUTION_TOO_SMALL: u32 = 6003;
pub const MAXIMUM_CONTRIBUTIONS_REACHED: u32 = 6004;
pub const FUNDRAISER_NOT_ENDED: u32 = 6005;
pub const FUNDRAISER_ENDED: u32 = 6006;
pub const INVALID_AMOUNT: u32 = 6007;

#[inline(always)]
pub fn custom(code: u32) -> ProgramError {
    ProgramError::Custom(code)
}
