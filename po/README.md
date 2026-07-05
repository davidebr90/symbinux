# Translations

The GUI is internationalised with GNU gettext. Source strings are English;
translations live here as `<language-code>.po` files and are compiled into
`src/symbinux/locale/<code>/LC_MESSAGES/symbinux.mo`.

Shipped today: English (source) and Italian (`it`). More can be added freely.

## Add or update a language

1. **Create the `.po`** (e.g. French):

   ```bash
   msginit --input=po/symbinux.pot --locale=fr --output=po/fr.po
   ```

   Then add `fr` to `po/LINGUAS`.

2. **Translate** — fill in each `msgstr` in `po/fr.po`.

3. **Compile** all languages to `.mo`:

   ```bash
   ./po/compile.sh
   ```

4. Run the app and pick the language from the menu (Language → …). To make a new
   language appear in that menu, add it to `NATIVE_LANGUAGES` in
   `src/symbinux/gui/i18n.py`.

## Refresh the template after code changes

When translatable strings change in the code, regenerate `symbinux.pot` and
merge it into existing translations:

```bash
xgettext --language=Python --keyword=_ --keyword=N_ --from-code=UTF-8 \
  --package-name=Symbinux --package-version=0.2.0 \
  --output=po/symbinux.pot src/symbinux/gui/*.py

for po in po/*.po; do msgmerge --update "$po" po/symbinux.pot; done
```

## Notes

- `_( )` translates at runtime; `N_( )` only marks a string for extraction
  (used for module-level tables), translated later with `_( )`.
- English needs no `.po`: gettext falls back to the source `msgid`.
