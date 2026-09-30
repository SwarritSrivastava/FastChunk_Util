use std::io;
use std::path::Path;

use crate::manifest::Manifest;

pub fn write_restore_scripts(_output_dir: &Path, _manifest: &Manifest) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub() {
        assert!(true);
    }
}
