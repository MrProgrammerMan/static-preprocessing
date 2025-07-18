use std::io;
use crate::File;

/// Renames a [`File`] based on the BLAKE3 hash of its contents.
///
/// This function calculates a BLAKE3 hash of the file's contents and uses it to
/// generate a new filename in the format `"{hash}.{ext}"`, preserving the original
/// file extension. The relative path is updated to reflect this new name while
/// keeping the parent directory intact.
///
/// # Parameters
///
/// - `file`: The [`File`] instance to rename. Ownership is taken since the returned
///   value modifies the original struct.
///
/// # Returns
///
/// A new [`File`] with an updated `relative_path`, or an [`io::Error`] if the file's
/// extension or parent directory is invalid.
///
/// # Examples
///
/// ```
/// # use std::path::{Path, PathBuf};
/// # use static_preprocessing::{File, FileType};
/// # use static_preprocessing::hash::hash_file_rename;
/// #
/// let file = File {
///     parent: Path::new("assets"),
///     relative_path: "css/main.css".into(),
///     file_type: FileType::CSS,
///     contents: b"body { margin: 0; }".to_vec(),
/// };
///
/// let renamed = hash_file_rename(file).unwrap();
/// assert!(renamed.relative_path.to_string_lossy().ends_with(".css"));
/// assert_eq!(renamed.relative_path, Path::new("css/057b37e61c8ec35690e7c0c321591990d37b9bdbef645cd780795a95672d65c0.css"));
/// ```
///
/// [`File`]: struct.File.html
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
pub fn hash_file_rename(file: File) -> Result<File, io::Error> {
    let hash = blake3::hash(file.contents.as_slice());
    let ext = file.relative_path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file extension."))?;
    let new_name = format!("{}.{}", hash.to_string(), ext);
    let new_relative = file.relative_path
        .parent()
        .map(|parent| parent.join(new_name))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No parent directory"))?;
    Ok(File {
        relative_path: new_relative,
        ..file
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use crate::FileType;

    #[test]
    fn test_hash_file_rename_valid_file() {
        let file = File {
            parent: Path::new("assets"),
            relative_path: "css/main.css".into(),
            file_type: FileType::CSS,
            contents: b"body { margin: 0; }".to_vec(),
        };

        let renamed = hash_file_rename(file).unwrap();
        assert!(renamed.relative_path.to_string_lossy().ends_with(".css"));
        assert_eq!(
            renamed.relative_path,
            Path::new("css/057b37e61c8ec35690e7c0c321591990d37b9bdbef645cd780795a95672d65c0.css")
        );
    }

    #[test]
    fn test_hash_file_rename_invalid_extension() {
        let file = File {
            parent: Path::new("assets"),
            relative_path: "css/main".into(), // No extension
            file_type: FileType::CSS,
            contents: b"body { margin: 0; }".to_vec(),
        };

        let result = hash_file_rename(file);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_hash_file_rename_empty_contents() {
        let file = File {
            parent: Path::new("assets"),
            relative_path: "css/main.css".into(),
            file_type: FileType::CSS,
            contents: vec![], // Empty contents
        };

        let renamed = hash_file_rename(file).unwrap();
        assert!(renamed.relative_path.to_string_lossy().ends_with(".css"));
        assert_eq!(
            renamed.relative_path,
            Path::new("css/af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262.css")
        );
    }
}