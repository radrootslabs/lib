//! UniFFI0.32.2 close policy. The upstream automatic AutoCloseable helper
//! collides with a real exported close method (upstream issue2955).
//! Only remove the handle-destruction alias for objects whose extracted Rust
//! metadata declares close; keep the source lifecycle call and destroy intact.
use anyhow::{Result, ensure};

const SHIM: &str = "    @Synchronized\n    override fun close() {\n        this.destroy()\n    }\n";

pub(super) fn preserve_close(source: &str, methods: &[(String, bool)]) -> Result<String> {
    let mut output = source.to_owned();
    for (name, asynchronous) in methods {
        let header = format!("open class {name}: Disposable, AutoCloseable, {name}Interface");
        ensure!(
            output.matches(&header).count() == 1,
            "unexpected Kotlin object header for {name}"
        );
        let start = output.find(&header).expect("counted header");
        let end = start
            + output[start..]
                .find("    internal inline fun <R> callWithHandle(")
                .ok_or_else(|| anyhow::anyhow!("unexpected Kotlin handle layout for {name}"))?;
        let prefix = &output[start..end];
        ensure!(
            prefix.matches(SHIM).count() == 1,
            "unexpected Kotlin close helper for {name}"
        );
        let method = if *asynchronous {
            "override suspend fun `close`()"
        } else {
            "override fun `close`()"
        };
        // Scope the exported method to this object, rather than another class.
        let tail_end = output[end..]
            .find("\nopen class ")
            .map_or(output.len(), |offset| end + offset);
        ensure!(
            output[end..tail_end].matches(method).count() == 1,
            "missing source close for {name}"
        );
        let replacement = prefix
            .replace(&header, &header.replace(", AutoCloseable", ""))
            .replacen(SHIM, "", 1);
        output.replace_range(start..end, &replacement);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn object(name: &str, asynchronous: bool) -> String {
        format!(
            "open class {name}: Disposable, AutoCloseable, {name}Interface\n{{\n    override fun destroy() {{ actualFree() }}\n{SHIM}\n    internal inline fun <R> callWithHandle(block: (Long) -> R): R {{ actualCall() }}\n    override {}fun `close`() {{ actualSourceClose() }}\n}}\n",
            if asynchronous { "suspend " } else { "" }
        )
    }
    #[test]
    fn preserves_actual_sync_and_async_close_and_other_objects() {
        let host = object("Host", false);
        let source = format!(
            "{}{}{}",
            object("NativeClient", true),
            object("Workspace", false),
            host
        );
        let actual = preserve_close(
            &source,
            &[("NativeClient".into(), true), ("Workspace".into(), false)],
        )
        .unwrap();
        assert!(actual.ends_with(&host));
        assert_eq!(actual.matches("actualSourceClose()").count(), 3);
        assert_eq!(actual.matches("actualFree()").count(), 3);
        assert_eq!(actual.matches(SHIM).count(), 1);
        assert!(!actual.contains("NativeClient: Disposable, AutoCloseable"));
        assert!(!actual.contains("Workspace: Disposable, AutoCloseable"));
    }
    #[test]
    fn rejects_drift_missing_source_and_reapplication() {
        let metadata = [("NativeClient".into(), true)];
        let source = object("NativeClient", true);
        assert!(preserve_close(&source.replace(SHIM, ""), &metadata).is_err());
        assert!(
            preserve_close(
                &source.replace("override suspend fun `close`", "fun other"),
                &metadata
            )
            .is_err()
        );
        assert!(preserve_close(&source.replace("callWithHandle", "changed"), &metadata).is_err());
        assert!(preserve_close(&format!("{source}{source}"), &metadata).is_err());
        assert!(preserve_close(&preserve_close(&source, &metadata).unwrap(), &metadata).is_err());
        assert_eq!(preserve_close(&source, &[]).unwrap(), source);
    }
}
