import AppKit
import SwiftUI

@main
struct DriftwatchApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        // First, so opening the app shows its window (the menu bar icon can be hidden
        // behind the notch or a menu bar manager).
        Window("Driftwatch", id: "main") {
            TimelineWindow(model: model)
        }
        .defaultSize(width: 980, height: 620)
        .commands {
            CommandMenu("Scan") {
                Button("Scan Now") { Task { await model.scan() } }
                    .keyboardShortcut("r")
                    .disabled(model.isScanning)
            }
            CommandGroup(after: .textEditing) {
                Button("Find…") { focusSearchField(in: NSApp.keyWindow) }
                    .keyboardShortcut("f")
            }
            CommandGroup(replacing: .help) {}
        }

        MenuBarExtra {
            MenuBarContent(model: model)
        } label: {
            Image(systemName: model.hasUnseenChanges ? "eye.trianglebadge.exclamationmark" : "eye")
                .accessibilityLabel(model.hasUnseenChanges ? "Driftwatch, new changes" : "Driftwatch")
        }
        .menuBarExtraStyle(.window)
    }
}

/// Bring the main window forward. Driftwatch lives in the menu bar, so the app
/// has to be activated for its window to come to the front.
@MainActor
func showMainWindow(_ openWindow: OpenWindowAction) {
    openWindow(id: "main")
    NSApp.activate()
}

/// Put the cursor in the window's toolbar search field (`.searchable` adds no ⌘F of its own).
@MainActor
func focusSearchField(in window: NSWindow?) {
    let search = window?.toolbar?.items.lazy.compactMap { $0 as? NSSearchToolbarItem }.first
    search?.beginSearchInteraction()
}
