# Темы терминала: откуда цвета

Таблица `THEMES` в `ui/js/v4/term-settings.js`. Файлы скачаны 23.10.2026 с `raw.githubusercontent.com`, значения
перенесены без правок, кроме отмеченного. Nightshift Dark/Light - своя таблица в `ui/js/v4/theme.js`.

| Тема | Источник | Что взято |
|---|---|---|
| Catppuccin Mocha | [catppuccin/kitty `themes/mocha.conf`](https://github.com/catppuccin/kitty/blob/main/themes/mocha.conf) (MIT) | разобрано импортом kitty: `background`, `foreground`, `cursor`, `selection_background`, `color0`-`color15` |
| Tokyo Night | [folke/tokyonight.nvim `extras/kitty/tokyonight_night.conf`](https://github.com/folke/tokyonight.nvim/blob/main/extras/kitty/tokyonight_night.conf) (Apache-2.0) | то же, импорт kitty |
| Dracula | [dracula/kitty `dracula.conf`](https://github.com/dracula/kitty/blob/master/dracula.conf) (MIT) | то же, импорт kitty |
| One Dark | [joshdick/onedark.vim `term/One Dark.itermcolors`](https://github.com/joshdick/onedark.vim/blob/main/term/One%20Dark.itermcolors) (MIT) | импорт iTerm2 (sRGB). Правка: `Selection Color` в файле равен цвету текста - выделение спрятало бы текст, взят `Ansi 8` (`#3e4452`) |
| Gruvbox Dark | [morhetz/gruvbox `colors/gruvbox.vim`](https://github.com/morhetz/gruvbox/blob/master/colors/gruvbox.vim) (MIT/X11 по README и package.json; файла LICENSE нет) | палитра `s:gb.*` и `g:terminal_color_0..15` для тёмного фона, контраст medium: фон `dark0`, текст `light1`, выделение `dark2` |
| Solarized Light | [altercation/solarized `README.md`](https://github.com/altercation/solarized/blob/master/README.md), таблица «SOLARIZED HEX / 16/8 TERMCOL» (MIT) | 16 цветов по столбцу TERMCOL; светлый фон: фон `base3`, текст `base00`, курсор `base01`, выделение `base2` |

Почему не `.itermcolors` для Gruvbox и Solarized: файлы `morhetz/gruvbox-contrib/iterm2/gruvbox-dark.itermcolors` и
`altercation/solarized/iterm2-colors-solarized/Solarized Light.itermcolors` записаны в калиброванном пространстве цвета,
их числа не совпадают с каноническими hex (`#1e1e1e` вместо `#282828`, `#fcf4dc` вместо `#fdf6e3`). Взяты канонические
значения из тех же репозиториев.

Проверка: `node scripts/ui-selftest.js` - импорт kitty/Ghostty/iTerm2 на образцах из этих файлов.
