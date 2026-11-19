-- Solo per la prova automatica su macOS (job "macos" della CI): nelle impostazioni del
-- salvaschermo, già aperte con "Custom", porta in fondo la lista (i salvaschermo di altri
-- sono in fondo e la lista crea solo le voci visibili), cerca la voce "ASCII Zoo" e la
-- sceglie. Serve il permesso Accessibilità (sul runner c'è).
tell application "System Events" to tell process "System Settings"
	set w to window 1
	repeat with e in (entire contents of w)
		try
			if class of e is scroll bar and (value of attribute "AXOrientation" of e) is "AXVerticalOrientation" then set value of e to 1.0
		end try
	end repeat
	delay 3
	set testi to {}
	repeat with e in (entire contents of w)
		set t to ""
		try
			set t to (name of e as text) & " " & (description of e as text)
		end try
		if t contains "ASCII Zoo" then
			try
				click e
			on error
				perform action "AXPress" of e
			end try
			return "scelto: " & (class of e as text) & " " & t
		end if
		try
			if class of e is static text then set end of testi to (value of e as text)
		end try
	end repeat
	set AppleScript's text item delimiters to " | "
	return "ASCII Zoo non trovato; testi: " & (testi as text)
end tell
