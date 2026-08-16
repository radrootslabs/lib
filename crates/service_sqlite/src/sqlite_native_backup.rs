//! Sealed online-backup calls over SQLx-owned locked SQLite handles.

use core::{fmt, marker::PhantomData, ptr::NonNull};
use std::error::Error;

use libsqlite3_sys as ffi;
use sqlx::sqlite::LockedSqliteHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeBackupStep {
    Done,
    More,
    Busy,
    Locked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeBackupFailureKind {
    Initialize,
    Step,
    Finish,
}

#[derive(PartialEq, Eq)]
pub(crate) struct NativeBackupError {
    kind: NativeBackupFailureKind,
    code: i32,
}

impl fmt::Debug for NativeBackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeBackupError")
            .field("kind", &self.kind)
            .field("code", &self.code)
            .finish()
    }
}

impl fmt::Display for NativeBackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            NativeBackupFailureKind::Initialize => "native SQLite backup initialization failed",
            NativeBackupFailureKind::Step => "native SQLite backup step failed",
            NativeBackupFailureKind::Finish => "native SQLite backup finalization failed",
        })
    }
}

impl Error for NativeBackupError {}

pub(crate) struct NativeBackup<'destination, 'source> {
    handle: Option<NonNull<ffi::sqlite3_backup>>,
    _locked_handles: PhantomData<(&'destination mut (), &'source mut ())>,
}

impl<'destination, 'source> NativeBackup<'destination, 'source> {
    pub(crate) fn start(
        destination: &'destination mut LockedSqliteHandle<'_>,
        source: &'source mut LockedSqliteHandle<'_>,
    ) -> Result<Self, NativeBackupError> {
        let destination = destination.as_raw_handle();
        let source = source.as_raw_handle();
        // SAFETY: both handles are borrowed from live SQLx lock guards, the
        // fixed schema names are valid NUL-terminated strings, and the backup
        // handle cannot outlive this function's returned owner.
        let handle = unsafe {
            ffi::sqlite3_backup_init(
                destination.as_ptr(),
                c"main".as_ptr(),
                source.as_ptr(),
                c"main".as_ptr(),
            )
        };
        let handle = NonNull::new(handle).ok_or_else(|| {
            // SAFETY: the destination SQLx guard keeps this handle live and
            // exclusively locked for the duration of the call.
            let code = unsafe { ffi::sqlite3_errcode(destination.as_ptr()) };
            NativeBackupError {
                kind: NativeBackupFailureKind::Initialize,
                code,
            }
        })?;
        Ok(Self {
            handle: Some(handle),
            _locked_handles: PhantomData,
        })
    }

    pub(crate) fn step(&mut self, pages: i32) -> Result<NativeBackupStep, NativeBackupError> {
        if pages <= 0 {
            return Err(NativeBackupError {
                kind: NativeBackupFailureKind::Step,
                code: ffi::SQLITE_MISUSE,
            });
        }
        let Some(handle) = self.handle else {
            return Err(NativeBackupError {
                kind: NativeBackupFailureKind::Step,
                code: ffi::SQLITE_MISUSE,
            });
        };
        // SAFETY: `handle` remains owned by this adapter and `pages` is a
        // positive bounded batch supplied by the capture driver.
        classify_step(unsafe { ffi::sqlite3_backup_step(handle.as_ptr(), pages) })
    }

    pub(crate) fn finish(mut self) -> Result<(), NativeBackupError> {
        let code = self.finish_once();
        if code == ffi::SQLITE_OK {
            Ok(())
        } else {
            Err(NativeBackupError {
                kind: NativeBackupFailureKind::Finish,
                code,
            })
        }
    }

    fn finish_once(&mut self) -> i32 {
        self.handle.map_or(ffi::SQLITE_OK, |handle| {
            self.handle = None;
            // SAFETY: taking the handle ensures exactly one finalization call.
            unsafe { ffi::sqlite3_backup_finish(handle.as_ptr()) }
        })
    }
}

impl Drop for NativeBackup<'_, '_> {
    fn drop(&mut self) {
        let _ = self.finish_once();
    }
}

fn classify_step(code: i32) -> Result<NativeBackupStep, NativeBackupError> {
    match code {
        ffi::SQLITE_DONE => Ok(NativeBackupStep::Done),
        ffi::SQLITE_OK => Ok(NativeBackupStep::More),
        ffi::SQLITE_BUSY => Ok(NativeBackupStep::Busy),
        ffi::SQLITE_LOCKED => Ok(NativeBackupStep::Locked),
        code => Err(NativeBackupError {
            kind: NativeBackupFailureKind::Step,
            code,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_codes_are_closed_and_stable() {
        assert_eq!(classify_step(ffi::SQLITE_DONE), Ok(NativeBackupStep::Done));
        assert_eq!(classify_step(ffi::SQLITE_OK), Ok(NativeBackupStep::More));
        assert_eq!(classify_step(ffi::SQLITE_BUSY), Ok(NativeBackupStep::Busy));
        assert_eq!(
            classify_step(ffi::SQLITE_LOCKED),
            Ok(NativeBackupStep::Locked)
        );
        assert_eq!(
            classify_step(ffi::SQLITE_CORRUPT)
                .expect_err("unexpected native code must fail")
                .kind,
            NativeBackupFailureKind::Step
        );
    }

    #[test]
    fn empty_and_misused_native_handles_fail_or_finish_deterministically() {
        let mut backup = NativeBackup {
            handle: None,
            _locked_handles: PhantomData,
        };
        for pages in [i32::MIN, -1, 0, 1] {
            let error = backup.step(pages).expect_err("invalid native handle");
            assert_eq!(error.kind, NativeBackupFailureKind::Step);
            assert_eq!(error.code, ffi::SQLITE_MISUSE);
        }
        backup.finish().expect("empty handle is already finalized");

        for (kind, expected) in [
            (
                NativeBackupFailureKind::Initialize,
                "native SQLite backup initialization failed",
            ),
            (
                NativeBackupFailureKind::Step,
                "native SQLite backup step failed",
            ),
            (
                NativeBackupFailureKind::Finish,
                "native SQLite backup finalization failed",
            ),
        ] {
            let error = NativeBackupError { kind, code: 1 };
            assert_eq!(error.to_string(), expected);
            assert!(format!("{error:?}").contains("code: 1"));
        }
    }
}
