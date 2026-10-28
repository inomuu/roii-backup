# Anime Home lobby

Implemented the approved landscape layout: central character artwork, profile upper left, live resource wallet upper right, left shortcuts, six bottom destinations and Adventure lower right. Existing routes and simulation-backed resource values are retained; the third resource is retained even though the concept only showed two. More retains secondary routes including Apothecary. Events uses the existing event screen instead of invented news content. The artwork is static, so character-switch arrows and invented profile progression are not displayed.

Asset: `unity/ROI/Assets/Resources/HomeUI/AnimeLobby.png`. Generated with built-in imagegen from the approved concept. Prompt: remove all UI/text/icons/counters and reconstruct scenery beneath; preserve central character, pose, framing and navy/violet castle balcony. Interactive controls are Unity UI, not baked into the artwork.

Validation: Home101 and Home101Mobile exercise navigation callbacks, overlay destinations, raycasts, language changes and the actual ModeTabs battle transition. Captures and results: `D:/roii/idle_mmorpg/ui-gallery/home-101/` and `home-101-mobile/`. Localization keys verified with tools/check_loc_keys.py. These are isolated presentation fixtures, not live account tests.
