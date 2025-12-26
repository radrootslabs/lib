#[typeshare::typeshare]
pub const KIND_PROFILE: u32 = 0;
#[typeshare::typeshare]
pub const KIND_POST: u32 = 1;
#[typeshare::typeshare]
pub const KIND_FOLLOW: u32 = 3;
#[typeshare::typeshare]
pub const KIND_REACTION: u32 = 7;
#[typeshare::typeshare]
pub const KIND_MESSAGE: u32 = 14;
#[typeshare::typeshare]
pub const KIND_COMMENT: u32 = 1111;
#[typeshare::typeshare]
pub const KIND_APP_DATA: u32 = 30078;
#[typeshare::typeshare]
pub const KIND_LISTING: u32 = 30402;
#[typeshare::typeshare]
pub const KIND_APPLICATION_HANDLER: u32 = 31990;

#[typeshare::typeshare]
pub const KIND_JOB_REQUEST_MIN: u32 = 5000;
#[typeshare::typeshare]
pub const KIND_JOB_REQUEST_MAX: u32 = 5999;
#[typeshare::typeshare]
pub const KIND_JOB_RESULT_MIN: u32 = 6000;
#[typeshare::typeshare]
pub const KIND_JOB_RESULT_MAX: u32 = 6999;
#[typeshare::typeshare]
pub const KIND_JOB_FEEDBACK: u32 = 7000;
