# convert -size 1024x1024 logo.svg test.png
convert -resize 256x256 -filter Sinc icon-1024.png icon-256.png
convert -resize 192x192 -filter Sinc icon-1024.png icon_ios_touch_192.png
convert -resize 512x512 -filter Sinc icon-1024.png maskable_icon_x512.png
optipng -o4 -clobber *.png