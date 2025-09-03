use std::io;
use crate::File;
use std::path::Path;

/// Renames a [`File`] based on the BLAKE3 hash of its contents.
///
/// This function calculates a BLAKE3 hash of the file's contents and uses it to
/// generate a new filename in the format `"{hash}.{ext}"`, preserving the original
/// file extension.
///
/// # Parameters
///
/// - `file`: The [`File`] instance to rename. Ownership is taken since the returned
///   value modifies the original struct.
///
/// # Returns
///
/// A new [`File`] with an updated `filename`, or an [`io::Error`] if the file's
/// extension is invalid.
///
/// # Examples
///
/// ```
/// # use static_preprocessing::{File, FileType};
/// # use static_preprocessing::hash::hash_file_rename;
/// #
/// let file = File {
///     filename: "main.css".to_string(),
///     file_type: FileType::CSS,
///     contents: b"body { margin: 0; }".to_vec(),
/// };
///
/// let renamed = hash_file_rename(file).unwrap();
/// assert!(renamed.filename.ends_with(".css"));
/// assert!(renamed.filename.len() > ".css".len());
/// ```
pub fn hash_file_rename(file: File) -> Result<File, io::Error> {
    let hash = blake3::hash(file.contents.as_slice());
    let ext = Path::new(&file.filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file extension."))?;
    let new_name = format!("{}.{}", hash.to_string(), ext);
    Ok(File {
        filename: new_name,
        ..file
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hash_file_rename() {
        use crate::File;
        use crate::FileType;

        let file = File {
            filename: "example.css".to_string(),
            file_type: FileType::CSS,
            contents: b"body { margin: 0; }".to_vec(),
        };

        let renamed = hash_file_rename(file).unwrap();

        // Verify the new filename ends with the correct extension
        assert!(renamed.filename.ends_with(".css"));

        // Verify the new filename contains a hash and is longer than just the extension
        assert!(renamed.filename.len() > ".css".len());

        // Verify the contents and file type remain unchanged
        assert_eq!(renamed.file_type, FileType::CSS);
        assert_eq!(renamed.contents, b"body { margin: 0; }");
    }
}