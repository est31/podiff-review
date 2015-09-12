## TODO

### Close to middle future

* support other ("metered gratis") translation APIs, like yandex
* treat problems (wrong user entries) better
* API-translate only the needed changes (the ones that aren't already answered with "Ok"). Saves translation quota, helps with large changesets.
* support the [po format](https://www.gnu.org/software/gettext/manual/html_node/PO-Files.html) better. The minimal support should be for what weblate spits out.
* perhaps add a "fast-forward" mode where if you have a change from not existing to untranslated, its accepted automatically. Would help with commits that add new languages, [example here](https://github.com/minetest/minetest/commit/0d1b41f3800d17915c4cbac86f6fbdc282b27aa4).
* allow user to edit the `reask_non_ok` flag
* ability to review multiple commits
* console color?

### Far future
*very likely to never ever happen*

* GUI frontend?
* Making it a generic crate, with the ability of other programs to use it
* adding tests :P
