import SwiftUI
import AppKit

@main
struct DymiumProviderApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @ObservedObject private var tokenService = TokenService.shared
    
    var body: some Scene {
        // Menu bar extra - the main (and only) UI
        MenuBarExtra {
            PopoverView(tokenService: tokenService)
        } label: {
            MenuBarIcon(state: tokenService.state)
        }
        .menuBarExtraStyle(.window)
        
        // Setup window - opened separately so it doesn't close with the popover
        Window("Dymium Setup", id: "setup") {
            SetupView(tokenService: tokenService, isPresented: .constant(true))
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .defaultPosition(.center)
    }
}

/// The menu bar icon - renders the ghost with the appropriate color
struct MenuBarIcon: View {
    let state: TokenState
    
    @State private var isPulsing = false
    
    var body: some View {
        Image(nsImage: createGhostImage())
            .opacity(opacity)
            .animation(pulseAnimation, value: isPulsing)
            .onAppear {
                startPulsingIfNeeded()
            }
            .onChange(of: state) { newState in
                isPulsing = newState.isAuthenticating
            }
    }
    
    private var opacity: Double {
        if state.isAuthenticating {
            return isPulsing ? 1.0 : 0.5
        }
        return 1.0
    }
    
    private var pulseAnimation: Animation? {
        if state.isAuthenticating {
            return .easeInOut(duration: 0.6).repeatForever(autoreverses: true)
        }
        return nil
    }
    
    private func startPulsingIfNeeded() {
        if state.isAuthenticating {
            isPulsing = true
        }
    }
    
    /// Create an NSImage of the ghost for the menu bar
    private func createGhostImage() -> NSImage {
        let size = NSSize(width: 18, height: 20)
        let image = NSImage(size: size, flipped: false) { rect in
            let color: NSColor
            switch self.state {
            case .idle:
                color = .systemGray
            case .authenticating:
                color = .systemYellow
            case .authenticated:
                color = .systemGreen
            case .failed:
                color = .systemRed
            }
            
            color.setFill()
            
            // Draw the ghost path
            let path = NSBezierPath()
            let width = rect.width
            let height = rect.height
            
            let domeHeight = height * 0.55
            
            // Start at bottom left
            path.move(to: CGPoint(x: 0, y: 0))
            
            // Wavy bottom with 3 scallops
            let scallops = 3
            let scallopWidth = width / CGFloat(scallops)
            let scallopDepth = height * 0.18
            
            for i in 0..<scallops {
                let startX = CGFloat(i) * scallopWidth
                let midX = startX + scallopWidth / 2
                let endX = startX + scallopWidth
                
                path.curve(
                    to: CGPoint(x: endX, y: 0),
                    controlPoint1: CGPoint(x: startX + scallopWidth * 0.25, y: scallopDepth),
                    controlPoint2: CGPoint(x: midX + scallopWidth * 0.25, y: scallopDepth)
                )
            }
            
            // Right side going up
            path.line(to: CGPoint(x: width, y: height - domeHeight))
            
            // Top dome (arc)
            path.appendArc(
                withCenter: CGPoint(x: width / 2, y: height - domeHeight),
                radius: width / 2,
                startAngle: 0,
                endAngle: 180,
                clockwise: false
            )
            
            // Left side going down
            path.line(to: CGPoint(x: 0, y: 0))
            
            path.close()
            path.fill()
            
            return true
        }
        
        image.isTemplate = false // We want our colors, not system tinting
        return image
    }
}

// MARK: - App Delegate for lifecycle handling

class AppDelegate: NSObject, NSApplicationDelegate {
    private var wakeObserver: NSObjectProtocol?
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Hide dock icon
        NSApp.setActivationPolicy(.accessory)
        
        // Start the token refresh loop
        Task { @MainActor in
            TokenService.shared.startRefreshLoop()
        }
        
        // Register for system wake notifications
        wakeObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didWakeNotification,
            object: nil,
            queue: .main
        ) { _ in
            Task { @MainActor in
                // Trigger a refresh when the system wakes from sleep
                await TokenService.shared.manualRefresh()
            }
        }
    }
    
    func applicationWillTerminate(_ notification: Notification) {
        TokenService.shared.stopRefreshLoop()
        
        // Remove wake observer
        if let observer = wakeObserver {
            NSWorkspace.shared.notificationCenter.removeObserver(observer)
        }
    }
}
