use std::path::Path;
use std::io;
use std::fs;

/// Recursively applies a function to each file in a directory tree.
///
/// This function traverses the directory specified by `p` and recursively
/// visits all its subdirectories. For each regular file encountered,
/// the provided closure `f` is called with a reference to the file's [`Path`].
///
/// # Arguments
///
/// * `p` - A reference to a [`Path`] representing the root directory to traverse.
/// * `f` - A mutable reference to a closure that takes a reference to a [`Path`] and returns `()`
///         (i.e., performs side effects such as logging, collecting paths, etc.).
///
/// # Returns
///
/// * [`Ok(())`] if the traversal completes successfully.
/// * [`Err`] if `p` is not a directory or if any I/O error occurs while reading directories.
///
/// # Errors
///
/// Returns an [`io::ErrorKind::NotADirectory`] error if `p` is not a directory.
/// Also propagates any errors returned by [`fs::read_dir`] or while accessing entries.
///
/// # Example
/// ```
/// 
///  # use std::fs;
///  # use std::path::PathBuf;
///  # use tempfile::TempDir;
///  # use static_preprocessing::for_each_file;
///  # /// Create temporary directory
///  # let temp_dir = TempDir::new().unwrap();
///  # let file0 = temp_dir.path().join("file0.txt");
///  # let sub_dir = temp_dir.path().join("sub");
///  # let sub_file = sub_dir.join("file1.jpeg");
/// 
///  # /// Create files
///  # fs::File::create(&file0).unwrap();
///  # fs::create_dir(&sub_dir).unwrap();
///  # fs::File::create(&sub_file).unwrap();
/// 
///  /// Capture visited paths
///  let mut visited: Vec<PathBuf> = Vec::new();
/// 
///  for_each_file(temp_dir.path(), &mut |p| visited.push(p.to_path_buf())).unwrap();
/// 
///  /// Check all files were visited
///  assert!(visited.contains(&file0));
///  assert!(visited.contains(&sub_file));
///  assert_eq!(visited.len(), 2);
/// ```
///
/// [`Path`]: std::path::Path
/// [`Ok(())`]: std::result::Result::Ok
/// [`Err`]: std::result::Result::Err
/// [`fs::read_dir`]: std::fs::read_dir
/// [`io::ErrorKind::NotADirectory`]: std::io::ErrorKind::NotADirectory
pub fn for_each_file<F: FnMut(&Path)>(p: &Path, f: &mut F) -> io::Result<()> {
    if p.is_dir() {
        for entry in fs::read_dir(p)? {
            let path = entry?.path();
            if path.is_dir() {
                for_each_file(&path, f)?;
            } else {
                f(&path);
            }
        }
        return Ok(());
    }
    Err(io::Error::new(io::ErrorKind::NotADirectory, "Not a directory"))
}