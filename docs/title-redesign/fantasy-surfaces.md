# Fantasy surface revision

Shared UI changes: round panel and border sprites, rounded retained-mesh frames, warm gold edges, indigo/violet primary actions and navy/violet window backdrops. The removed Menu/Inventory top strip stays hidden; Home and the mode-switch navigation remain available. Existing layouts, controls and data are retained.

Production C# compilation passed. New captures are being collected using the existing gallery fixtures. Validation is not clean: Unity 6000.6.3f1 reports a MainToolbarWindow UIElements layout recursion ("Layout update failed to stabilize after 10 iterations"). Restarting the editor and requesting its Default layout did not resolve it. Capture result error counts include this editor failure and must not be described as passed tests. Results: D:/roii/unified-ui-validation.txt. Some character portraits remain unavailable, including Suzu; these are independent asset gaps, not resolved by this visual change.
