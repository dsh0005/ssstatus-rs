# ssstatus-rs
A super simple statusbar widget for e.g. Sway.

## The name
Choose your own:
- Storm's simple status(bar)
- Super simple status(bar)
- Silly simple status(bar)
- (Your preference goes here)

## What?
It's the simplest statusbar widget that satisfies my needs:
- Display the time, to the minute.
- Display the battery charge level, to the percent.
- Always be up to date, as much as possible.
- Use few resources.
- Scratch all my other itches.

This leads to a list of "don't"s:
- No polling. We use reads, timers, `poll(2)`/`epoll(2)`/&c. here. We
  wake up exactly when (modulo the scheduler &al.) it is necessary, no
  earlier, no later.
- Don't try to read the battery ourselves. Funny things happen
  sometimes, and we don't want to be in that business. Just ask UPower
  about the battery.
- Don't try to detect the timezone change ourselves, ask systemd about
  it.

### Fun and subtle things that came up
Some fun and subtle problems came up while implementing this. Here are
a few of them:
- Figuring out how long it is until the beginning of the next minute is
  hard if not impossible. It's complicated by the possibility of clock
  adjustments, timezone changes, leap seconds, suspend-to-RAM/disk, and
  probably more.
- The last time a country officially used a timezone with a UTC offset
  that wasn't a multiple of 60 seconds (that I can find) was Liberia in
  1972. Check the tz database for more info.
- Sometimes your battery just winks out of existance for about 40
  milliseconds.
- Detecting clock adjustments is (mostly) pretty easy, as long as
  you're okay with the occasional false positive.
- swaywm/sway#4496

## Platform requirements and assumptions
- `timerfd`s. We need these to detect clock adjustments.
- DBus. We use it to listen for timezone and battery changes.
    - UPower. We ask it for the battery percentage, then listen for
      property changes from it.
    - systemd. We use `org.freedesktop.timedate1` to get the timezone
      and listen for changes.
- We assume that all currently-used timezones have a UTC offset that is
  a multiple of 60 seconds.

## Leftover bugs, unimplemented things, and future directions
- If UPower ever invalidates the battery percentage, we don't handle
  that case. We'll probably need to spawn a task to go fetch the data.
  The same goes for the timezone.
- The way `dbus` works clashes with how I want to do async. The
  matchers are pretty gross, and should make that obvious.
- Tokio wants to offload writes to stdout to a worker thread. It's fine
  since I can set the keepalive to more than my expected update
  interval, but it could probably be better.
- We set timerslack to a reasonable value of 7.5 ms. We should really
  consider querying sway to see if we can figure out a better guess.

## License
AGPLv3 (only), refer to `LICENSE.txt` for more info. I wrote the whole
thing myself (so far).
