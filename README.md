# git-fixme
A little git helper that displays where FIXMEs are in your code. 

### Additional options 

--insertion    

This will list the commit hash which was introducing the keyword in the source file.


--file

This will only report the files where the keywords are part of.

--stats

This will print how many keywords are spread accross how many files.
Output format: Files Fixmes

## Other keywords than FIXMEs

You can use a env var called GIT_FIXME_KEYS to specify the keywords which are used to scan the file.
The following example will highlight all lines of code which have a a or b in them.

Example:
GIT_FIXME_KEYS=A:B


