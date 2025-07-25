## 0.0.4 - 2025-07-25
 - many new features and bug fixes
 - some error messages have been improved

### New features
 - Walrus Sites is now supported. Install it using `suiup install site-builder`
 - `suiup cleanup` will clean the cache directory. Use `--help` to see the available flags
 - `suiup switch` now allows to switch binary versions, including different nightly versions
 - `suiup doctor` will check the environment information for issues
 - `suiup` can now be installed from script into a custom directory. See the docs for more information

### Bug fixes
 - the check for newer suiup version has been fixed

## 0.0.3 - 2025-07-07
 - refactorings
 - bug fixes

## 0.0.2 - 2025-06-02

 - supports now to pass = or == or @ for specifying a version
 - new self command to make it easy to self update: `suiup self update`.
 - enabled tracing to be able to run with RUST_LOG=info suiup
 - lots of code refactoring thanks to @wangeguo. 

## 0.0.1 - 2025-03-09

- First release.
