# Pain points: the official MasterHUB app and similar decks

What users complain about in the software of the Cooler Master MasterHUB and comparable controllers,
and what HubDeck should do about it. Researched on 2026-10-10 by web search: tech press reviews,
vendor help pages and release notes, vendor and community forums (Adobe, OBS, DCS, Razer Insider,
Keyboard Maestro, SoundFlow), GitHub issue trackers, Trustpilot and retailer reviews.

Limits: Reddit could not be read by the research tools (blocked for search and fetch), so
"how often" is a judgement from the other sources. GitHub counts: `c` = comments, `r` = reactions
(read 2026-10-10). "(snippet)" means only the search result snippet was seen, not the page. Every
pain point has an id (MH1, SD3, ...) used in the last section.

## Cooler Master MasterHUB (official app)

User-level reports are scarce: the product sold mostly through Kickstarter (691 backers) and a few
retailers; Newegg had no reviews. Most evidence comes from press reviews. The official app is still
released (v1.0.0.113, 2026-05-27, [download page](https://www.coolermaster.com/en-global/software-masterhub.html)).

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **MH1** Faders drive only a fixed short list of targets: "only four specific controls can currently be mapped"; you "can't even use all five" sliders | some (headline con of 2 reviews) | [Progressive Robot](https://blog.progressiverobot.com/cooler-master-masterhub-review-a-modular-stream-deck-with-potential), [Tom's Hardware](https://www.tomshardware.com/peripherals/docking-stations-hubs/cooler-master-masterhub-review) (snippet) |
| **MH2** Hotkeys must be typed on a physical keyboard, "no dropdown menu to select available keys" (numpad, keys past F12); people record macros on other devices to work around it | some (2 reviews) | [Tom's Hardware](https://www.tomshardware.com/peripherals/docking-stations-hubs/cooler-master-masterhub-review) (snippet), [Progressive Robot](https://blog.progressiverobot.com/cooler-master-masterhub-review-a-modular-stream-deck-with-potential) |
| **MH3** Settings lost: launching the app "would wipe all my changes"; "commands did not save properly", "lose a full setup"; "remember to save things, or they will disappear" | some (4 reviews) | [APH Networks p.2](https://aphnetworks.com/reviews/cooler-master-masterhub/2), [APH p.3](https://aphnetworks.com/reviews/cooler-master-masterhub/3), [CGMagazine](https://www.cgmagonline.com/review/hardware/cooler-master-masterhub-review/), [GamingTrend](https://gamingtrend.com/reviews/cooler-master-masterhub-review-the-voltron-of-stream-decks/) |
| **MH4** Crashes and instability ("some crashes and odd issues", "improve the stability"); release notes keep fixing connection, installer and copy-profile bugs | some | [CGMagazine](https://www.cgmagonline.com/review/hardware/cooler-master-masterhub-review/), [APH p.3](https://aphnetworks.com/reviews/cooler-master-masterhub/3), [release notes](https://www.coolermaster.com/en-au/masterhub-release-notes.html) |
| **MH5** Deck dead without the app: actions work "only if the software utility is running"; on app close the hardware sits in "a continuous loading loop" | few (1 review, strong) | [APH p.2](https://aphnetworks.com/reviews/cooler-master-masterhub/2) |
| **MH6** Narrow integrations (OBS, Streamlabs, Twitch, Adobe); no Audacity, DaVinci Resolve or other streaming platforms; no plugin library, the promised marketplace never came | many (most reviews) | [APH p.2](https://aphnetworks.com/reviews/cooler-master-masterhub/2), [Digital Trends via Yahoo](https://tech.yahoo.com/computing/articles/didn-t-think-needed-modular-180042111.html), [Progressive Robot](https://blog.progressiverobot.com/cooler-master-masterhub-review-a-modular-stream-deck-with-potential), [GamingTrend](https://gamingtrend.com/reviews/cooler-master-masterhub-review-the-voltron-of-stream-decks/) |
| **MH7** Generic actions and no presets: "a lot of the functionality here is generalized", "I'd love to see ... presets", "a bit barebones" | some | [GamingTrend](https://gamingtrend.com/reviews/cooler-master-masterhub-review-the-voltron-of-stream-decks/), [Digital Trends via Yahoo](https://tech.yahoo.com/computing/articles/didn-t-think-needed-modular-180042111.html) |
| **MH8** LEDs per module only: "all three dials in a single unit will share the same color"; "rather few effects"; no color per function | some (4 reviews) | [Progressive Robot](https://blog.progressiverobot.com/cooler-master-masterhub-review-a-modular-stream-deck-with-potential), [Noisy Pixel](https://noisypixel.net/cooler-master-masterhub-review-streamdeck-competitor/), [Impulse Gamer](https://www.impulsegamer.com/coolermaster-masterhub-review/), [GamingTrend](https://gamingtrend.com/reviews/cooler-master-masterhub-review-the-voltron-of-stream-decks/) |
| **MH9** Knob modes invisible: only the dial screen shows what a control does; with the 3-knob module users must "remember" the mode | few | [Impulse Gamer](https://www.impulsegamer.com/coolermaster-masterhub-review/) |
| **MH10** Confusing UI: "disorganized", features "hard to find", "digging through layers of options", "overwhelming and confusing"; a turn-off for beginners | some (3 reviews) | [Noisy Pixel](https://noisypixel.net/cooler-master-masterhub-review-streamdeck-competitor/), [Yanko Design](https://www.yankodesign.com/2024/10/05/cooler-master-masterhub-controller-review-complete-control-over-your-creative-pursuits/), [GamingTrend](https://gamingtrend.com/reviews/cooler-master-masterhub-review-the-voltron-of-stream-decks/) |
| **MH11** Macros "overcomplex" (multi-task and hotkeys combined) | few | [Noisy Pixel](https://noisypixel.net/cooler-master-masterhub-review-streamdeck-competitor/) |
| **MH12** Icon editor "rather simple"; app icons not imported for launchers ("should automatically import any application icons") | few | [Noisy Pixel](https://noisypixel.net/cooler-master-masterhub-review-streamdeck-competitor/), [APH p.3](https://aphnetworks.com/reviews/cooler-master-masterhub/3) |
| **MH13** Layout view wrong: modules over the edge of the base "scrunch together" in the software; orientation confused when the base is turned | few | [GamingTrend](https://gamingtrend.com/reviews/cooler-master-masterhub-review-the-voltron-of-stream-decks/), [Noisy Pixel](https://noisypixel.net/cooler-master-masterhub-review-streamdeck-competitor/) |
| **MH14** Module disconnections ("sensitivity and frequency of module disconnections"); problems traced to outdated firmware or software | few | [Noisy Pixel](https://noisypixel.net/cooler-master-masterhub-review-streamdeck-competitor/) |
| **MH15** OBS connection trouble ("trouble connecting my local server to OBS") | few | [Noisy Pixel](https://noisypixel.net/cooler-master-masterhub-review-streamdeck-competitor/) |
| **MH16** Slow updates and fear of abandonment: "lagged on the promised software updates", "Assuming Cooler Master doesn't abandon it completely"; a retailer page calls it "notoriously buggy" | some | [Tom's Hardware](https://www.tomshardware.com/peripherals/docking-stations-hubs/cooler-master-masterhub-review) (snippet), [B&H](https://www.bhphotovideo.com/c/product/1846578-REG/cooler_master_mhsk13aa00_masterhub.html) (snippet) |
| **MH17** Windows only in practice: "Windows 10 (64-bit) or later" is the only stated requirement | few | [download page](https://www.coolermaster.com/en-global/software-masterhub.html) |
| **MH18** Heavy background daemon: on the HubDeck owner's PC, installing 1.0.0.113 froze Windows within ~3 minutes (likely `MHDaemon` polling CPU/GPU sensors through a kernel driver) | own observation (1) | [docs/KNOWLEDGE.md](KNOWLEDGE.md#the-official-app-do-not-install-it-again-casually) |

## Elgato Stream Deck (incl. Stream Deck +)

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **SD1** The app must always run; without it the deck goes "unconnected" and "pressing any button results in nothing happening" | many (by design) | [igorslab](https://igorslab.de/en/elgato-stream-deck-in-test-oled-touchscreen-knobs-tool-for-streamer-and-not-streamer/4), [KM forum](https://forum.keyboardmaestro.com/t/a-question-for-stream-deck-users/22420) (snippet) |
| **SD2** General reliability: "won't always connect or stay connected"; Studio: "changes are not properly synchronized", "Only an outdated version ... runs stably" | some | [AlternativeTo](https://alternativeto.net/software/elgato-stream-deck-mobile/about/), [Thomann reviews](https://www.thomannmusic.ch/elgato_stream_deck_studio_reviews.htm), [Trustpilot](https://nl.trustpilot.com/review/www.elgato.com?page=2) (snippet) |
| **SD3** Elgato account login needed for Marketplace installs and plugin updates (lock-in; few direct complaints) | few | [Elgato help: updating plugins](https://help.elgato.com/hc/en-us/articles/360050819312-Elgato-Stream-Deck-Software-Updating-Plugins-and-Integrations) (snippet) |
| **SD4** An expired Elgato signing certificate stopped plugins; the fix was to log in to the Marketplace and update | many (hit everyone with those plugins) | [Verge story, mirror](https://rss.boorghani.com/?p=363772) (snippet) |
| **SD5** Auto profile switching: off while the editor is open, per app not per window, needs workarounds; "very inconsistent" | some | [KM forum](https://forum.keyboardmaestro.com/t/profiles-not-switching-for-applications/16963), [OBS forum](https://obsproject.com/forum/threads/obs-plugin-for-streamdeck-wont-switch-profiles.182311/), [MPU](https://talk.macpowerusers.com/t/my-experiences-with-uses-of-stream-deck-pedal/28010) (snippets) |
| **SD6** Hotkeys don't reach apps running as administrator (games, OBS); running Stream Deck elevated "is not an option within Stream Deck" | many | [Zwift forum](https://forums.zwift.com/t/stream-deck-peripheral-stopped-working-after-latest-zwift-update/15621), [OBS forum](https://obsproject.com/forum/threads/hotkeys-not-working-properly.69137/page-2) (snippets) |
| **SD7** Hotkeys in games: numpad keys arrive as plain keys, multi-modifier combos fail, NumLock/Break/SysRq unmappable | some (sim communities) | [DCS 299006](https://forum.dcs.world/topic/299006-stream-deck-hotkey-numpad-keys-do-not-work-in-dcs/), [DCS 300669](https://forum.dcs.world/topic/300669-streamdeck-xl-no-sending-some-keyboard-commands), [IL-2 forum](https://forum.il2sturmovik.com/topic/54936-question-for-anyone-who-uses-an-elgato-stream-deck) (snippets) |
| **SD8** F13-F24 used to avoid clashes, but some apps ignore them (F24 "does nothing" in OBS hotkeys) | few | [OBS forum](https://obsproject.com/forum/threads/any-way-to-use-f13-f24-keys-for-shortcuts-using-a-stream-deck.143192/) (snippet) |
| **SD9** macOS permissions break Hotkey and Text actions (Accessibility; Local Network after Sequoia) | some | [Elgato help](https://help.elgato.com/hc/en-us/articles/360028240651-Elgato-Stream-Deck-Fixing-Hotkeys-and-Text-actions-in-macOS-10-14-Mojave), [Ecamm](https://support.ecamm.com/en/articles/9977416-stream-deck-no-longer-controlling-ecamm-after-updating-to-sequoia) (snippets) |
| **SD10** Plugins break after app or plugin updates ("completely broken", "failed to load"); Elgato sends users to the plugin author | many | [Elgato help](https://help.elgato.com/hc/en-us/articles/15894171000333-Elgato-Stream-Deck-Plugin-Not-Working), [HWiNFO forum](https://www.hwinfo.com/forum/threads/latest-public-release-breaks-stream-deck-plugin.7535/), [7.0.1 notes](https://help.elgato.com/hc/en-us/articles/39330355624977-Elgato-Stream-Deck-7-0-1-Release-Notes) (snippets) |
| **SD11** CPU and memory leaks come from plugins (VoiceMod, HWiNFO, Wirecast), not the main app | some | [direwolf20](https://x.com/direwolf20/status/1172194082711642113?lang=en), [HWiNFO leak](https://www.hwinfo.com/forum/threads/memory-leak.8669/) (snippets) |
| **SD12** Update regressions: startup crash on macOS (fixed 7.0.3), freeze on page switch, profile import bugs (fixed 7.1.1) | some | [7.0.3 notes](https://help.elgato.com/hc/en-us/articles/40016247300369-Elgato-Stream-Deck-7-0-3-Release-Notes), [7.1.1 notes](https://help.elgato.com/hc/en-us/articles/42210974537233-Elgato-Stream-Deck-7-1-1-Release-Notes) (snippets) |
| **SD13** Stream Deck + dials and touch strip: "Limited dial and touch bar support", strip tied to its dial, no long press in dial stacks, touches hit the wrong dial | many (reviews) | [PCGamesN](https://www.pcgamesn.com/elgato-stream-deck-plus-review), [Kowatek](https://blog.kowatek.com/?p=1758) |
| **SD14** No sync between computers ("Wish I could just have a live config file ... synced"); backups came back blank after a move | some | [KM forum](https://forum.keyboardmaestro.com/t/if-i-buy-a-new-stream-deck/31154), [vMix forum](https://forums.vmix.com/posts/t24932-Shortcuts-don-t-work-El-Gato-Streamdeck-after-moving-to-laptop--SOLVED), [Windows Report](https://windowsreport.com/stream-deck-profile-disappeared/) (snippets) |
| **SD15** Folder Back button fixed in the top-left key ("taking up the most valuable space"); a third-party editor exists to move it | some | [SoundFlow forum](https://forum.soundflow.org/-5078/move-folder-back-button), [BarRaider Folder Editor](https://github.com/BarRaider/streamdeck-foldereditor) (snippets) |
| **SD16** Multi-actions can't nest and have no conditions | few | [Elgato help](https://help.elgato.com/hc/en-us/articles/7092527123725) (snippet) |
| **SD17** USB disconnects and freezes; after sleep the deck stays dead until apps are reopened | some | [SoundFlow help](https://help.soundflow.org/en/articles/5761177-troubleshooting-stream-deck-freezing-connectivity-issues), [Elgato help: blank keys](https://help.elgato.com/hc/en-us/articles/14006356917773-Elgato-Stream-Deck-Keys-show-Elgato-logo-or-appear-blank) (snippets) |
| **SD18** No official Linux support | some | [Lawrence Systems forum](https://forums.lawrencesystems.com/t/elgato-stream-deck-with-linux/1361), [GamingOnLinux](https://www.gamingonlinux.com/2019/10/using-the-elgato-stream-deck-on-linux-just-got-a-whole-lot-easier-with-streamdeck-ui/page=1/) (snippets) |
| **SD19** Marketplace: hard search, paid profiles (~$30 for Adobe) imported by hand; third-party plugins run with full privileges, sellers not vetted | some | [PCGamesN](https://www.pcgamesn.com/elgato-stream-deck-plus-review), [Kowatek](https://blog.kowatek.com/?p=1758), [Gearspace](https://gearspace.com/threads/warning-about-streamdeck.1461556/) (snippet) |
| **SD20** Mobile app subscription (free tier 6 keys): "Not working. asking payment." | few | [Elgato help: Mobile Pro](https://help.elgato.com/hc/en-us/articles/19662381509261-Stream-Deck-Mobile-Pro) (snippet), [AlternativeTo](https://alternativeto.net/software/elgato-stream-deck-mobile/about/) |
| **SD21** Screensaver and dimming need plugins and work only while the app runs; turning the deck off on Mac sleep "not possible" | few | [BarRaider sdScreensaver](https://barraider.com/sdscreensaver), [SoundFlow forum](https://forum.soundflow.org/-6010/streamdeck-never-going-to-screensaver-when-computer-is-asleep) (snippets) |

## Razer Stream Controller (Loupedeck software, Synapse)

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **RZ1** Razer plugins (Key Light, OBS, Discord, Twitch) never updated for Loupedeck 6.x; vendor advice: stay on 5.9.1; "this issue never was resolved" | many (long-running) | [Razer Insider 72170](https://insider.razer.com/razer-support-45/stream-controller-x-continued-support-72170) |
| **RZ2** Abandoned: "Razer's completely abandoned this hardware", "I basically have brick with useless buttons"; OBS plugin "deletes itself on launch" | many (strong) | [Insider 72170](https://insider.razer.com/razer-support-45/stream-controller-x-continued-support-72170), [Insider 87050](https://insider.razer.com/general-discussion-6/stream-controller-honest-answer-from-razer-87050) |
| **RZ3** No working version: old 5.9.1 client can't log in ("outdated authentication"), 6.x lacks the Razer plugins | some (strong) | [Insider 82352](https://insider.razer.com/razer-support-45/razer-stream-controller-82352) |
| **RZ4** Support asks for personal data and locks threads, no public plan | some | [Insider 82352](https://insider.razer.com/razer-support-45/razer-stream-controller-82352) |
| **RZ5** Synapse 4 plugin broken; "reverting back to a previous version of synapse seems to be intentionally prevented" | few | [Insider 88431](https://insider.razer.com/razer-support-45/razer-stream-controller-by-loupedeck-88431) |
| **RZ6** Device support announced to be dropped in Loupedeck 6.4 (May 2026), reversed in August after feedback | many (every owner) | [loupedeck.com notice](https://loupedeck.com/?p=12269) |
| **RZ7** Logi Options+ 2.4-2.6 shipped a shared plugin service without Razer support and broke the device | some | [loupedeck.com notice](https://loupedeck.com/?p=12269) |
| **RZ8** Migration to 6.0: plugins reinstalled, some missing, profiles with "broken actions" | some | [Loupedeck 6.0 release](https://support.loupedeck.com/loupedeck-6.html) (snippet) |
| **RZ9** Slow editor: "wait literally minutes at some points for a mouse click to register" | some (reviews) | [Well Played](https://www.well-played.com.au/razer-stream-controller-review/), [ZDNet](https://www.zdnet.com.au/home-and-office/home-entertainment/razer-stream-controller-review/) (snippet) |
| **RZ10** Synapse adds little but is heavy (~25 processes, ~2 GB; RazerAppEngine 18.3 GB after a night) | some | [Well Played](https://www.well-played.com.au/razer-stream-controller-review/), [Insider: Synapse 4 memory](https://insider.razer.com/razer-support-45/synapse-4-and-chroma-app-extreme-memory-usage-89612) (snippet) |

## Loupedeck and Logitech MX Creative Console (Logi Options+)

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **LD1** Forced 6.0 move to the shared Logi platform "has caused issues for many users" (vendor apology): plugins reinstalled, third-party plugins incompatible, missing app profiles and actions | many (vendor admits) | [Loupedeck apology](https://loupedeck.com/?p=12177), [6.0 changes](https://loupedeck.com/loupedeck-software-update-6-0/) |
| **LD2** Version lock-in: the Loupedeck app "is incompatible with Logitech Options+" on 5.9.1; users disable Options+ auto-update | some | [loupedeck.com notice](https://loupedeck.com/?p=12269), [Loupedeck support](https://support.loupedeck.com/loupedeck-software-does-no-longer-work-after-installing-options) (snippet) |
| **LD3** End of life: hardware sales ended 2025-03; legacy 5.8 software stopped on 2026-03-30 because "the required cloud services are no longer available" | many (structural) | [5.8 end of support](https://loupedeck.com/?p=12261), [end of sales](https://loupedeck.com/?p=12214) (snippet) |
| **LD4** Premiere Pro v25 removed Loupedeck control-surface support; "i can't make it work again" | some (17 replies) | [Adobe ideas thread](https://community.adobe.com/t5/premiere-pro-ideas/the-new-update-removed-support-for-the-quot-loupedeck-control-surface-quot/idi-p/14917880) |
| **LD5** Lightroom Classic integration fragile: stops after LrC restarts ("Only a reboot gets it going again"); dial "refuses to change any of the Lightroom sliders" | some (for years) | [Adobe LrC 2025](https://community.adobe.com/t5/lightroom-classic-discussions/lightroom-classic-on-imac-with-loupedeck/m-p/15440275), [Adobe 2017](https://community.adobe.com/questions-675/loupedeck-problems-since-lightroom-classic-cc-990378), [Amateur Photographer](https://amateurphotographer.com/review/logitech-mx-creative-console-review) |
| **LD6** Dial latency and lost steps on fast turns ("after a few ticks nothing happens") | some | [CommandPost #2350](https://github.com/CommandPost/CommandPost/issues/2350) |
| **LD7** Photoshop: every dial tick is a History state ("set Exposure to 50, you get 50 states") | few | [DPReview comment](https://www.dpreview.com/members/7944834332/comments) (snippet) |
| **LD8** Options+ mandatory, needs an account with date of birth ("unnecessarily intrusive") | some | [Amateur Photographer](https://amateurphotographer.com/review/logitech-mx-creative-console-review) |
| **LD9** Options+ "buggy and a massive resource hog": 2-4 GB RAM, "1000+ threads", "occasionally prevents shutdown" | some (strong) | [Target reviews](https://www.target.com/p/logitech-mx-creative-console-9-customizable-buttons-with-lcd-screens-graphite/-/A-1006718799), [V2EX](https://www.v2ex.com/t/963969) |
| **LD10** Per-app profile switching drops out ("drop out of the Lightroom profile for no obvious reason"; Figma not switched) | some | [Amateur Photographer](https://amateurphotographer.com/review/logitech-mx-creative-console-review), [Desertcart](https://www.desertcart.co.za/products/658988172) (snippet) |
| **LD11** Clumsy editor: no copy/paste between pages or profiles, paging doesn't wrap, tiny text, an "AI Prompt Builder" nobody asked for | some | [ProVideo Coalition](https://www.provideocoalition.com/review-logitech-mx-creative-console/) |
| **LD12** Shared config bug: changing the MX dial also changed a Loupedeck CT's dial | few | [ProVideo Coalition](https://www.provideocoalition.com/review-logitech-mx-creative-console/) |
| **LD13** Apps without a deep integration: "tedious set-up and no support for community-created setups" | some | [Target reviews](https://www.target.com/p/logitech-mx-creative-console-9-customizable-buttons-with-lcd-screens-graphite/-/A-1006718799) |
| **LD14** An expired certificate broke Options+ on macOS; the auto-updater couldn't fix it ("We dropped the ball") | many (mouse users) | [The Register](https://www.theregister.com/2026/01/08/logitech_mouse_chaos/) |
| **LD15** No Linux support | few | [openSUSE Hack Week](https://hackweek.opensuse.org/projects/support-loupedeck-ct-hardware-on-linux) |

## TourBox (TourBox Console)

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **TB1** Lightroom Classic plugin hangs LrC on quit ("narrowed it down to the Tourbox Console Plugin") | some | [Adobe LrC bug thread](https://community.adobe.com/t5/lightroom-classic-bugs/p-unable-to-relaunch-after-file-gt-new-catalog-switching-catalogs-or-relaunching-with-certain/idi-p/15457714/page/2) |
| **TB2** Plugin's localhost TCP link fails ("failed to make a connection"), suspected port clash with the Loupedeck plugin | few | [Adobe thread](https://community.adobe.com/t5/lightroom-classic-discussions/internal-error-tourbox-needs-acces-to-tcp-ports-but-failed-to-make-a-connection/m-p/15186377) |
| **TB3** Premiere plugin crashes Premiere when opening Preferences | some | [Adobe 2024](https://community.adobe.com/t5/premiere-pro-discussions/premiere-crashes-everytime-i-open-settings/td-p/14893691/page/2), [Adobe 2023](https://community.adobe.com/t5/premiere-pro-discussions/premiere-crashes-when-i-go-to-preferences-v23-v24-and-v24-betas-mac-13-5-2/td-p/14138239) |
| **TB4** Host app updates break or slow it ("When Studio upgraded to version 21 the Box stopped working"; "sluggish to the point of being unusable") | some | [Trustpilot](https://www.trustpilot.com/review/tourboxtech.com), [Adobe bug report](https://community.adobe.com/bug-reports-733/this-build-made-the-tourbox-sluggish-to-the-point-of-being-unusable-906397) |
| **TB5** Weak integration outside the main Adobe apps (Camera Raw sliders, non-Adobe apps) | some | [Trustpilot](https://www.trustpilot.com/review/tourboxtech.com) |
| **TB6** App must run; check it "EVERY TIME the Mac wakes up from sleep", sometimes replug; multi-step macOS permissions | few (old) | [digitalEPIGRAPHY](https://www.digital-epigraphy.com/reviews/a-capable-photoshop-remote-for-shortcut-champions-tourbox-controller-review) |
| **TB7** Learning curve: combos to memorise ("you'll still need a good memory") | many (reviews) | [Digital Camera World](https://www.digitalcameraworld.com/reviews/tourbox-review) (snippet) |
| **TB8** Support doesn't answer ("No response what so ever") | some | [Trustpilot](https://www.trustpilot.com/review/tourboxtech.com) |

## Monogram Creative Console

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **MG1** Company went dark: no closure statement, orders not shipped, refunds only via chargebacks | many (strong) | [torley](https://torley.substack.com/p/the-uplifting-life-and-dastardly), [Trustpilot](https://www.trustpilot.com/review/monogramcc.com), [Fractal Audio forum](https://forum.fractalaudio.com/threads/monogram-cc-caution.202510/) |
| **MG2** Abandoned software breaks with host updates ("failed to open localhost:50110" in Lightroom); auth servers offline; MIDI mode + MIDI2LR as the workaround | some (growing) | [Rob Futrell guide](https://robfutrell.com/monogram-creative-console-lightroom-fix/) |
| **MG3** Hardware useless without the vendor app: "does not function without this app active, which could be troubling if the company shutters" | some | [geekhack](https://geekhack.org/index.php?topic=112685.0) |
| **MG4** Error-prone editor: "There is no Cancel option, and Clear will wipe out your work" | few | [ProVideo Coalition](https://www.provideocoalition.com/review-monogram-creative-console/) |
| **MG5** Missing integrations (Avid, Resolve; Unreal removed) | some | [ProVideo Coalition](https://www.provideocoalition.com/review-monogram-creative-console/), [DPReview](https://www.dpreview.com/reviews/review-monogram-creative-console-a-refinement-of-our-favorite-modular-editing-hardware) |
| **MG6** Creator won't start after install or update (old process still running) | some (own FAQ) | [Monogram support](https://support.monogramcc.com/hc/en-us/articles/360053668791-Monogram-Creator-will-not-run-after-installation-or-update) (snippet) |

## deej (Arduino volume mixer)

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **DJ1** A Windows Update of the CH340 serial driver stopped it (values arrive, deej does nothing or closes); some BSODs | many (top 3 issues, c=27 r=18) | [#127](https://github.com/omriharel/deej/issues/127), [#129](https://github.com/omriharel/deej/issues/129), [#130](https://github.com/omriharel/deej/issues/130) |
| **DJ2** No reconnect after sleep or replug; one user restarts it "every five minutes" from Task Scheduler | many (c=16 and more) | [#146](https://github.com/omriharel/deej/issues/146), [#47](https://github.com/omriharel/deej/issues/47), [#38](https://github.com/omriharel/deej/issues/38) |
| **DJ3** Starts too early at boot and never finds the device | some (c=13) | [#23](https://github.com/omriharel/deej/issues/23) |
| **DJ4** Keeps the PC awake or wakes it: noisy pots keep sending values | some (c=12, c=8) | [#52](https://github.com/omriharel/deej/issues/52), [#142](https://github.com/omriharel/deej/issues/142) |
| **DJ5** Slider noise and range: jumps "from 0 to 100 to 0", ends never reach 0 or 100 % | some | [#43](https://github.com/omriharel/deej/issues/43), [#20](https://github.com/omriharel/deej/issues/20), [#107](https://github.com/omriharel/deej/issues/107) |
| **DJ6** No GUI, YAML config; wrong COM port and it "open[s] and close[s] quickly without any error messages" | some | [#85](https://github.com/omriharel/deej/issues/85), [#96](https://github.com/omriharel/deej/issues/96), [#48](https://github.com/omriharel/deej/issues/48) |
| **DJ7** Some apps' audio sessions not controlled (Windows Media Player, TeamSpeak, Tidal, some mics) | some (many small issues) | [#1](https://github.com/omriharel/deej/issues/1), [#53](https://github.com/omriharel/deej/issues/53), [#26](https://github.com/omriharel/deej/issues/26) |
| **DJ8** No macOS (r=17) | some | [#7](https://github.com/omriharel/deej/issues/7) |
| **DJ9** Missing: buttons (mute, push-to-talk), output device switching, more than one controller | some | [#108](https://github.com/omriharel/deej/issues/108), [#22](https://github.com/omriharel/deej/issues/22), [#122](https://github.com/omriharel/deej/issues/122) |
| **DJ10** "seems abandoned" (last push 2024-07); users move to forks | some | [#148](https://github.com/omriharel/deej/issues/148) |

## OpenDeck and StreamController (open Stream Deck software, Linux)

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **OD1** Input simulation on Wayland: permission prompt on every login; Write Text types wrong characters on non-US layouts | many (c=36, c=35) | [OD#171](https://github.com/nekename/OpenDeck/issues/171), [OD#120](https://github.com/nekename/OpenDeck/issues/120), [OD#287](https://github.com/nekename/OpenDeck/issues/287), [SC#191](https://github.com/StreamController/StreamController/issues/191) |
| **OD2** Profile switching by active window missing or flaky ("the only thing I'm missing from Windows") | many (c=14, c=27) | [OD#27](https://github.com/nekename/OpenDeck/issues/27), [SC#215](https://github.com/StreamController/StreamController/issues/215), [SC#256](https://github.com/StreamController/StreamController/issues/256) |
| **OD3** "No devices detected" at start, not found until replug, keys dead after sleep | many (c=30, c=18) | [OD#24](https://github.com/nekename/OpenDeck/issues/24), [SC#459](https://github.com/StreamController/StreamController/issues/459), [SC#112](https://github.com/StreamController/StreamController/issues/112) |
| **OD4** RAM grows with pages, leak while locked, idle CPU over 10 % after an update | some (c=56) | [SC#123](https://github.com/StreamController/StreamController/issues/123), [SC#203](https://github.com/StreamController/StreamController/issues/203), [SC#433](https://github.com/StreamController/StreamController/issues/433) |
| **OD5** Writes SVGs and the whole profile to disk on every image change ("this is poorly designed") | some | [OD#249](https://github.com/nekename/OpenDeck/issues/249) |
| **OD6** Delays shutdown, blocks sleep; requests to blank the deck when the screen locks | some | [OD#261](https://github.com/nekename/OpenDeck/issues/261), [SC#458](https://github.com/StreamController/StreamController/issues/458), [OD#248](https://github.com/nekename/OpenDeck/issues/248) |
| **OD7** Updates break it ("utterly unusable", runtime updates) | some (c=18, c=28) | [SC#437](https://github.com/StreamController/StreamController/issues/437), [SC#529](https://github.com/StreamController/StreamController/issues/529), [SC#477](https://github.com/StreamController/StreamController/issues/477) |
| **OD8** Elgato plugin compatibility: Windows plugins need Wine, Node not found, plugin events overwrite custom icons | some | [OD#1](https://github.com/nekename/OpenDeck/issues/1), [OD#229](https://github.com/nekename/OpenDeck/issues/229), [OD#152](https://github.com/nekename/OpenDeck/issues/152) |
| **OD9** App watcher floods D-Bus up to a desktop crash | few (c=23) | [OD#425](https://github.com/nekename/OpenDeck/issues/425) |
| **OD10** Private data in log files; hold / long-press requested | some | [SC#439](https://github.com/StreamController/StreamController/issues/439), [SC#110](https://github.com/StreamController/StreamController/issues/110), [OD#254](https://github.com/nekename/OpenDeck/issues/254) |

## Mountain DisplayPad (Base Camp)

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **MP1** Windows only; community ports for Linux, macOS and the browser; one calls the product "EOL" | many | [KitGuru](https://www.kitguru.net/peripherals/mat-mynett/mountain-displaypad-and-macropad-review/), [mountain-displaypad-mac](https://github.com/ChipNeedham/mountain-displaypad-mac), [displaypad-pwa](https://github.com/justb81/mountain-displaypad-pwa) |
| **MP2** "needs time to mature", "Slightly laggy/unresponsive at times" | some | [KitGuru](https://www.kitguru.net/peripherals/mat-mynett/mountain-displaypad-and-macropad-review/) |
| **MP3** Sending logs to the vendor on by default (Base Camp, reviewed on a Mountain keyboard) | few | [IgorsLab](https://www.igorslab.de/mountain-everest-max-tastatur-review/5/) |
| **MP4** Slow image upload, stale preview, settings apply "after several attempts" | few | [IgorsLab](https://www.igorslab.de/mountain-everest-max-tastatur-review/5/) |
| **MP5** Launcher takes only `.exe`; users wrap PowerShell scripts as exes | few | [DisplayPad-Scripts](https://github.com/TheNerdMan/DisplayPad-Scripts) |
| **MP6** SDK: profile switch returns true but does nothing; state unreadable; rapid image uploads glitch | few | [SDK#5](https://github.com/Mountain-BC/DisplayPad.SDK.Demo/issues/5), [SDK#7](https://github.com/Mountain-BC/DisplayPad.SDK.Demo/issues/7), [SDK#9](https://github.com/Mountain-BC/DisplayPad.SDK.Demo/issues/9) |

## Ulanzi, Ajazz, Mirabox and other Stream Dock clones

| Pain point | How often / how strongly | Source(s) |
|---|---|---|
| **CL1** Only key release reported, so no hold or long press ("inferior and confusing user experience") | some | [companion#3072](https://github.com/bitfocus/companion/issues/3072), [Device-SDK#90](https://github.com/MiraboxSpace/StreamDock-Device-SDK/issues/90) |
| **CL2** Stream Deck plugins requiring Node.js don't run | some (c=35) | [Plugin-SDK#10](https://github.com/MiraboxSpace/StreamDock-Plugin-SDK/issues/10) |
| **CL3** Official plugins break (Spotify, Discord OAuth "plugin is dead", rate limits) | many in that repo | [Plugins#17](https://github.com/MiraboxSpace/StreamDock-Plugins/issues/17), [#21](https://github.com/MiraboxSpace/StreamDock-Plugins/issues/21), [#51](https://github.com/MiraboxSpace/StreamDock-Plugins/issues/51) |
| **CL4** Trust: software "super sketchy", Defender flags a Trojan, closed `libtransport.so` | some | [teardown](https://karlquinsland.com/aliexpress-11-11-sale-teardowns-2024/), [Plugins#69](https://github.com/MiraboxSpace/StreamDock-Plugins/issues/69), [Device-SDK#16](https://github.com/MiraboxSpace/StreamDock-Device-SDK/issues/16) |
| **CL5** Vendor FAQ: driver must always run, freezes, CPU temperature only as admin, "fix" by turning off UAC, scene switching only when minimized, one device per model | some (vendor's own FAQ) | [Stream Dock FAQ](https://support.key123.vip/faqs/streamDock.html) |
| **CL6** Untranslated add-ons; users can't add UI languages | few | [AJAZZ-AKP153R-RUS](https://github.com/Aleksey1998/AJAZZ-AKP153R-RUS) |
| **CL7** Ulanzi: account needed to download profiles ("Did I just buy this paperweight to give this company my email"); images duplicated in AppData; small plugin catalog | few to some | [UlanziSDK#4](https://github.com/UlanziTechnology/UlanziDeckPlugin-SDK/issues/4), [CGMagazine](https://www.cgmagonline.com/review/hardware/ulanzi-stream-deck-d200/), [Fstoppers](https://fstoppers.com/reviews/ulanzi-d200x-and-dial-review-can-they-improve-your-editing-workflow-900870) |
| **CL8** No Linux or open software; a D200 Linux manager restores the layout "after a firmware UI crash" | many (several projects) | [opendeck-akp153](https://github.com/4ndv/opendeck-akp153), [ulanzi-d200-linux](https://github.com/racerxdl/ulanzi-d200-linux) |

## What HubDeck should do

Tags: **[done]** HubDeck does it per `README.md` and `docs/ROADMAP.md` (several items are built but
still wait for the owner's hardware test, `[~]` in the roadmap); **[planned]** on the roadmap;
**[new]** not on the roadmap yet. The ids in brackets are the pain points each item cures.

### [new], by value

1. **Automatic profile backups with history**: keep the last N versions of every profile and the
   settings, a "Restore" list in the UI, a backup before every import, update or migration. (MH3,
   SD14, LD1, RZ8, MG4)
2. **Input that reaches games and elevated apps**: send scan codes, hold each key a configurable
   few ms, keep numpad keys distinct, support NumLock/Pause/SysRq; detect an elevated window in
   front and offer an optional elevated input helper instead of running the whole app as admin.
   (SD6, SD7, SD8, CL5)
3. **Plugin watchdog**: show CPU and memory per plugin, restart a crashed plugin, stop a runaway
   one, never let a plugin freeze the deck. (SD10, SD11, OD4, LD9, RZ10)
4. **Stable, versioned plugin API**: old plugins keep working across HubDeck updates; an action
   whose plugin is missing keeps its settings and shows a "plugin missing" look instead of being
   dropped. (RZ1, RZ8, LD1, LD4, SD10, MG2, CL3)
5. **Update control**: updates opt-in with a changelog, the previous version kept, downgrade
   allowed, profiles readable by older versions or migrated with a backup. (SD12, OD7, LD2, RZ5,
   RZ7)
6. **Import official MasterHUB profiles (`.mhz`)** so switching costs nothing, if the format can be
   read (to be checked on a profile the owner exports). (MH3, MH16)
7. **Idle and lock behaviour**: dim or blank keys and the dial after idle or when Windows locks,
   brightness settings for the key panel and the dial, never block sleep or shutdown. (SD21, OD6,
   DJ4)
8. **Fader calibration**: deadzone and hysteresis, ends that reach exactly 0 and 100 %, no events
   while a fader rests. (DJ4, DJ5, MH1)
9. **Profile switching rules by window title** (not only by app), switching that stays on while
   the editor is open (with a toggle), and a "lock this profile" option. (SD5, OD2, LD10)
10. **Settings sync folder**: keep profiles in a folder the user picks (OneDrive, Dropbox, git) so
    two PCs share one setup, with conflict handling. (SD14)
11. **Diagnostics bundle**: one click collects the log, versions and module list, with window
    titles and tokens scrubbed, ready for an issue. (MH4, SD2, OD10)
12. **App icons for "Open app"**: take the icon from the exe or `.app` automatically. (MH12)
13. **Audio device actions**: switch default output and input device; volume targets "app in
    front" and "everything not mapped". (DJ7, DJ9)
14. **Type text on any keyboard layout** (Unicode input; check with the Polish layout). (OD1)
15. **Run scripts and commands** with arguments, working folder and hidden window (PowerShell,
    `.bat`, shell). (MP5)
16. **Footprint budget**: idle CPU and memory targets checked by a long-run (soak) test before each
    release; profile files written only when they change, never on image updates. (MH18, LD9, OD4,
    OD5, RZ10)
17. **Multi-action logic**: toggles, conditions on a state (muted, OBS recording), nested
    sequences. (SD16, MH11)
18. **Merge fast dial turns into one change** for apps that record every step in their history.
    (LD7)
19. **Plugin permissions in the manifest** (network, files, processes) shown before a plugin is
    enabled. (SD19, CL4)
20. **More than one base or deck at once.** (DJ9, CL5) Not planned: the owner decided against it (2026-10-11).

### [planned]

- macOS build with clear permission guidance (Accessibility, Input Monitoring). (MH17, SD9, DJ8,
  MP1, TB6)
- Linux (optional). (SD18, LD15, MP1, CL8, OD3)
- Stream Deck / OpenAction plugin compatibility where concepts match, core actions behind the same
  plugin interface. (MH6, CL2, OD8, LD13)
- Code signing (no antivirus false alarms) and the updater; CI tests on Windows and macOS against
  regressions. (CL4, SD12, OD7)
- Public repository with an issue tracker: support and fixes do not depend on a vendor. (RZ4, TB8,
  MG1, MH16)
- Community slot data for module positions not yet verified. (MH13)

### [done]

- No account, no cloud, no online check at run time, open source (MIT OR Apache-2.0): the software
  cannot be switched off, abandoned or locked behind a login. Keep it so (code signing must never
  make the app stop when a certificate expires). (SD3, SD4, LD3, LD8, LD14, RZ2, RZ3, MG1, MG2, MG3,
  CL7, DJ10, MH16)
- Faders, knobs and the dial drive any numeric target: system, app and microphone volume, OBS
  mixer, MIDI CC, monitor brightness, Home Assistant lights. (MH1, SD13)
- Hotkeys picked from a list incl. F13-F24 and media keys; recording optional. (MH2, SD8)
- Profiles written atomically; editor with undo, copy/paste, drag and drop; export/import. (MH3,
  MG4, LD11, SD14)
- Runs in the tray, starts with the system, keeps working with the window closed; hands keys and
  dial back to their own animation when paused, quit or asleep and reopens after resume. (MH5, SD1,
  SD17, TB6, DJ2)
- Hot-plug of modules, reconnect, arrange mode with the base turn override, module restart on the
  "not responding" banner. (MH13, MH14, OD3, DJ3)
- Per-app profile switching (Windows; WinEvent hook plus a 250 ms check). (SD5, LD10, OD2)
- Pages that work like folders, with "Go to page" on any key. (SD15)
- Tap, hold and double press on keys, knob pushes and the dial. (CL1, OD10)
- Multi-actions with delays. (MH11)
- LED color per knob and fader, brightness following the value, base edge color per profile.
  (MH8)
- Dial modes with their names on the dial screen; values shown for 1.5 s after a change. (MH9)
- Presets: everyday desk, OBS streaming, Discord and meetings, Photoshop, Lightroom, video editing,
  game action bar; first-run welcome. (MH7, MH10, LD13)
- OBS over obs-websocket with live state; Home Assistant; MIDI output (works with MIDI2LR as a
  universal fallback); now playing. (MH6, MH15, MG2, MG5)
- Plugins in any language over a local WebSocket and a local API. (MH6, MP6)
- System monitor without a kernel driver and without admin rights. (MH18, CL5)
- Dial turns merged per batch, only the newest state drawn (no backlog or lag). (LD6)
- Own pictures, animated GIFs, full-panel picture or clock. (MH12)
- Rotating log file. (MH4)
- English UI with Polish as an option; texts in one place for more languages. (CL6)
