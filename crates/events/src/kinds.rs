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

#[inline]
pub const fn is_request_kind(kind: u32) -> bool {
    kind >= KIND_JOB_REQUEST_MIN && kind <= KIND_JOB_REQUEST_MAX
}
#[inline]
pub const fn is_result_kind(kind: u32) -> bool {
    kind >= KIND_JOB_RESULT_MIN && kind <= KIND_JOB_RESULT_MAX
}
#[inline]
pub const fn result_kind_for_request_kind(kind: u32) -> Option<u32> {
    if is_request_kind(kind) {
        Some(kind + 1000)
    } else {
        None
    }
}
#[inline]
pub const fn request_kind_for_result_kind(kind: u32) -> Option<u32> {
    if is_result_kind(kind) {
        Some(kind - 1000)
    } else {
        None
    }
}
