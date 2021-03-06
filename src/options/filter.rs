//! Parsing the options for `FileFilter`.

use fs::DotFilter;
use fs::filter::{FileFilter, SortField, SortCase, IgnorePatterns};

use options::{flags, Misfire};
use options::parser::MatchedFlags;


impl FileFilter {

    /// Determines which of all the file filter options to use.
    pub fn deduce(matches: &MatchedFlags) -> Result<FileFilter, Misfire> {
        Ok(FileFilter {
            list_dirs_first: matches.has(&flags::DIRS_FIRST)?,
            reverse:         matches.has(&flags::REVERSE)?,
            sort_field:      SortField::deduce(matches)?,
            dot_filter:      DotFilter::deduce(matches)?,
            ignore_patterns: IgnorePatterns::deduce(matches)?,
        })
    }
}

const SORTS: &[&str] = &[ "name", "Name", "size", "extension",
                          "Extension", "modified", "accessed",
                          "created", "inode", "type", "none" ];

impl SortField {

    /// Determines which sort field to use based on the `--sort` argument.
    /// This argument’s value can be one of several flags, listed above.
    /// Returns the default sort field if none is given, or `Err` if the
    /// value doesn’t correspond to a sort field we know about.
    fn deduce(matches: &MatchedFlags) -> Result<SortField, Misfire> {
        let word = match matches.get(&flags::SORT)? {
            Some(w)  => w,
            None     => return Ok(SortField::default()),
        };

        // The field is an OsStr, so can’t be matched.
        if word == "name" || word == "filename" {
            Ok(SortField::Name(SortCase::Sensitive))
        }
        else if word == "Name" || word == "Filename" {
            Ok(SortField::Name(SortCase::Insensitive))
        }
        else if word == "size" || word == "filesize" {
            Ok(SortField::Size)
        }
        else if word == "ext" || word == "extension" {
            Ok(SortField::Extension(SortCase::Sensitive))
        }
        else if word == "Ext" || word == "Extension" {
            Ok(SortField::Extension(SortCase::Insensitive))
        }
        else if word == "mod" || word == "modified" {
            Ok(SortField::ModifiedDate)
        }
        else if word == "acc" || word == "accessed" {
            Ok(SortField::AccessedDate)
        }
        else if word == "cr" || word == "created" {
            Ok(SortField::CreatedDate)
        }
        else if word == "inode" {
            Ok(SortField::FileInode)
        }
        else if word == "type" {
            Ok(SortField::FileType)
        }
        else if word == "none" {
            Ok(SortField::Unsorted)
        }
        else {
            Err(Misfire::bad_argument(&flags::SORT, word, SORTS))
        }
    }
}

impl Default for SortField {
    fn default() -> SortField {
        SortField::Name(SortCase::Sensitive)
    }
}


impl DotFilter {

    /// Determines the dot filter based on how many `--all` options were
    /// given: one will show dotfiles, but two will show `.` and `..` too.
    ///
    /// It also checks for the `--tree` option in strict mode, because of a
    /// special case where `--tree --all --all` won't work: listing the
    /// parent directory in tree mode would loop onto itself!
    pub fn deduce(matches: &MatchedFlags) -> Result<DotFilter, Misfire> {
        let count = matches.count(&flags::ALL);

        if count == 0 {
            Ok(DotFilter::JustFiles)
        }
        else if count == 1 {
            Ok(DotFilter::Dotfiles)
        }
        else if matches.count(&flags::TREE) > 0 {
            Err(Misfire::TreeAllAll)
        }
        else if count >= 3 && matches.is_strict() {
            Err(Misfire::Conflict(&flags::ALL, &flags::ALL))
        }
        else {
            Ok(DotFilter::DotfilesAndDots)
        }
    }
}


impl IgnorePatterns {

    /// Determines the set of glob patterns to use based on the
    /// `--ignore-patterns` argument’s value. This is a list of strings
    /// separated by pipe (`|`) characters, given in any order.
    pub fn deduce(matches: &MatchedFlags) -> Result<IgnorePatterns, Misfire> {

        // If there are no inputs, we return a set of patterns that doesn’t
        // match anything, rather than, say, `None`.
        let inputs = match matches.get(&flags::IGNORE_GLOB)? {
            None => return Ok(IgnorePatterns::empty()),
            Some(is) => is,
        };

        // Awkwardly, though, a glob pattern can be invalid, and we need to
        // deal with invalid patterns somehow.
        let (patterns, mut errors) = IgnorePatterns::parse_from_iter(inputs.to_string_lossy().split('|'));

        // It can actually return more than one glob error,
        // but we only use one. (TODO)
        match errors.pop() {
            Some(e) => Err(e.into()),
            None    => Ok(patterns),
        }
    }
}



#[cfg(test)]
mod test {
    use super::*;
    use std::ffi::OsString;
    use options::flags;
    use options::parser::Flag;

    pub fn os(input: &'static str) -> OsString {
        let mut os = OsString::new();
        os.push(input);
        os
    }

    macro_rules! test {
        ($name:ident: $type:ident <- $inputs:expr; $stricts:expr => $result:expr) => {
            #[test]
            fn $name() {
                use options::parser::Arg;
                use options::test::parse_for_test;
                use options::test::Strictnesses::*;

                static TEST_ARGS: &[&Arg] = &[ &flags::SORT, &flags::ALL, &flags::TREE, &flags::IGNORE_GLOB ];
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| $type::deduce(mf)) {
                    assert_eq!(result, $result);
                }
            }
        };
    }

    mod sort_fields {
        use super::*;

        // Default behaviour
        test!(empty:         SortField <- [];                  Both => Ok(SortField::default()));

        // Sort field arguments
        test!(one_arg:       SortField <- ["--sort=cr"];       Both => Ok(SortField::CreatedDate));
        test!(one_long:      SortField <- ["--sort=size"];     Both => Ok(SortField::Size));
        test!(one_short:     SortField <- ["-saccessed"];      Both => Ok(SortField::AccessedDate));
        test!(lowercase:     SortField <- ["--sort", "name"];  Both => Ok(SortField::Name(SortCase::Sensitive)));
        test!(uppercase:     SortField <- ["--sort", "Name"];  Both => Ok(SortField::Name(SortCase::Insensitive)));

        // Errors
        test!(error:         SortField <- ["--sort=colour"];   Both => Err(Misfire::bad_argument(&flags::SORT, &os("colour"), super::SORTS)));

        // Overriding
        test!(overridden:    SortField <- ["--sort=cr",       "--sort", "mod"];     Last => Ok(SortField::ModifiedDate));
        test!(overridden_2:  SortField <- ["--sort", "none",  "--sort=Extension"];  Last => Ok(SortField::Extension(SortCase::Insensitive)));
        test!(overridden_3:  SortField <- ["--sort=cr",       "--sort", "mod"];     Complain => Err(Misfire::Duplicate(Flag::Long("sort"), Flag::Long("sort"))));
        test!(overridden_4:  SortField <- ["--sort", "none",  "--sort=Extension"];  Complain => Err(Misfire::Duplicate(Flag::Long("sort"), Flag::Long("sort"))));
    }


    mod dot_filters {
        use super::*;

        // Default behaviour
        test!(empty:      DotFilter <- [];               Both => Ok(DotFilter::JustFiles));

        // --all
        test!(all:        DotFilter <- ["--all"];        Both => Ok(DotFilter::Dotfiles));
        test!(all_all:    DotFilter <- ["--all", "-a"];  Both => Ok(DotFilter::DotfilesAndDots));
        test!(all_all_2:  DotFilter <- ["-aa"];          Both => Ok(DotFilter::DotfilesAndDots));

        test!(all_all_3:  DotFilter <- ["-aaa"];         Last => Ok(DotFilter::DotfilesAndDots));
        test!(all_all_4:  DotFilter <- ["-aaa"];         Complain => Err(Misfire::Conflict(&flags::ALL, &flags::ALL)));

        // --all and --tree
        test!(tree_a:     DotFilter <- ["-Ta"];          Both => Ok(DotFilter::Dotfiles));
        test!(tree_aa:    DotFilter <- ["-Taa"];         Both => Err(Misfire::TreeAllAll));
        test!(tree_aaa:   DotFilter <- ["-Taaa"];        Both => Err(Misfire::TreeAllAll));
    }


    mod ignore_patternses {
        use super::*;
        use std::iter::FromIterator;
        use glob;

        fn pat(string: &'static str) -> glob::Pattern {
            glob::Pattern::new(string).unwrap()
        }

        // Various numbers of globs
        test!(none:   IgnorePatterns <- [];                             Both => Ok(IgnorePatterns::empty()));
        test!(one:    IgnorePatterns <- ["--ignore-glob", "*.ogg"];     Both => Ok(IgnorePatterns::from_iter(vec![ pat("*.ogg") ])));
        test!(two:    IgnorePatterns <- ["--ignore-glob=*.ogg|*.MP3"];  Both => Ok(IgnorePatterns::from_iter(vec![ pat("*.ogg"), pat("*.MP3") ])));
        test!(loads:  IgnorePatterns <- ["-I*|?|.|*"];                  Both => Ok(IgnorePatterns::from_iter(vec![ pat("*"), pat("?"), pat("."), pat("*") ])));

        // Overriding
        test!(overridden:   IgnorePatterns <- ["-I=*.ogg",    "-I", "*.mp3"];  Last => Ok(IgnorePatterns::from_iter(vec![ pat("*.mp3") ])));
        test!(overridden_2: IgnorePatterns <- ["-I", "*.OGG", "-I*.MP3"];      Last => Ok(IgnorePatterns::from_iter(vec![ pat("*.MP3") ])));
        test!(overridden_3: IgnorePatterns <- ["-I=*.ogg",    "-I", "*.mp3"];  Complain => Err(Misfire::Duplicate(Flag::Short(b'I'), Flag::Short(b'I'))));
        test!(overridden_4: IgnorePatterns <- ["-I", "*.OGG", "-I*.MP3"];      Complain => Err(Misfire::Duplicate(Flag::Short(b'I'), Flag::Short(b'I'))));
    }
}
