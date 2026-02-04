import SwiftUI
import AppKit

// MARK: - NSColor extensions for Dymium brand colors

extension NSColor {
    /// Dymium primary blue #4369ff
    static let dymiumPrimary = NSColor(red: 0x43/255, green: 0x69/255, blue: 0xff/255, alpha: 1.0)
    /// Dymium accent purple #904dff
    static let dymiumAccent = NSColor(red: 0x90/255, green: 0x4d/255, blue: 0xff/255, alpha: 1.0)
}

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

/// The menu bar icon - renders the ghost with the appropriate style
/// - Authenticated: accent-primary gradient outline
/// - Authenticating: rotating/morphing gradient outline  
/// - Idle/Failed: gray outline
struct MenuBarIcon: View {
    let state: TokenState
    
    @State private var gradientPhase: CGFloat = 0
    
    var body: some View {
        Image(nsImage: createGhostImage())
            .onAppear {
                if state.isAuthenticating {
                    startGradientAnimation()
                }
            }
            .onChange(of: state) { newState in
                if newState.isAuthenticating {
                    startGradientAnimation()
                }
            }
    }
    
    private func startGradientAnimation() {
        // Animation is handled in the NSImage drawing for menu bar
        // SwiftUI animations don't work well with NSImage-based menu bar icons
    }
    
    /// Create an NSImage of the ghost for the menu bar (outlined/stroked)
    private func createGhostImage() -> NSImage {
        let size = NSSize(width: 18, height: 24)
        let image = NSImage(size: size, flipped: false) { rect in
            let strokeWidth: CGFloat = 1.5
            let insetRect = rect.insetBy(dx: strokeWidth / 2 + 1, dy: strokeWidth / 2 + 1)
            
            // Create the ghost path
            let path = self.createGhostPath(in: insetRect)
            
            // Set up stroke based on state
            switch self.state {
            case .idle, .failed:
                // Gray outline
                NSColor.systemGray.setStroke()
                path.lineWidth = strokeWidth
                path.lineCapStyle = .round
                path.lineJoinStyle = .round
                path.stroke()
                
            case .authenticating, .authenticated:
                // Draw subtle white glow for contrast
                NSGraphicsContext.saveGraphicsState()
                NSColor.white.withAlphaComponent(0.4).setStroke()
                path.lineWidth = strokeWidth + 2
                path.stroke()
                NSGraphicsContext.restoreGraphicsState()
                
                // Draw gradient using multiple segments
                // Split the path into sections and color each
                self.drawGradientStroke(path: path, in: insetRect, strokeWidth: strokeWidth)
            }
            
            return true
        }
        
        image.isTemplate = false // We want our colors, not system tinting
        return image
    }
    
    /// Draw a gradient stroke by drawing the path multiple times with clipping
    private func drawGradientStroke(path: NSBezierPath, in rect: NSRect, strokeWidth: CGFloat) {
        let height = rect.height + rect.minY * 2
        let segments = 10  // More segments = smoother gradient
        let segmentHeight = height / CGFloat(segments)
        
        path.lineWidth = strokeWidth
        path.lineCapStyle = .round
        path.lineJoinStyle = .round
        
        for i in 0..<segments {
            let t = CGFloat(i) / CGFloat(segments - 1)
            
            // Interpolate between accent (purple) and primary (blue)
            let color = interpolateColor(from: NSColor.dymiumAccent, to: NSColor.dymiumPrimary, t: t)
            
            // Create clip rect for this segment (from top to bottom)
            let clipY = height - CGFloat(i + 1) * segmentHeight
            let clipRect = NSRect(x: 0, y: clipY, width: rect.width + rect.minX * 2, height: segmentHeight + 1)
            
            NSGraphicsContext.saveGraphicsState()
            NSBezierPath(rect: clipRect).addClip()
            color.setStroke()
            path.stroke()
            NSGraphicsContext.restoreGraphicsState()
        }
    }
    
    /// Interpolate between two NSColors
    private func interpolateColor(from: NSColor, to: NSColor, t: CGFloat) -> NSColor {
        // Convert to RGB color space
        guard let fromRGB = from.usingColorSpace(.deviceRGB),
              let toRGB = to.usingColorSpace(.deviceRGB) else {
            return from
        }
        
        let r = fromRGB.redComponent + (toRGB.redComponent - fromRGB.redComponent) * t
        let g = fromRGB.greenComponent + (toRGB.greenComponent - fromRGB.greenComponent) * t
        let b = fromRGB.blueComponent + (toRGB.blueComponent - fromRGB.blueComponent) * t
        
        return NSColor(red: r, green: g, blue: b, alpha: 1.0)
    }
    
    /// Create the ghost bezier path
    private func createGhostPath(in rect: NSRect) -> NSBezierPath {
        let path = NSBezierPath()
        let width = rect.width
        let height = rect.height
        
        let domeRadius = width / 2
        let bodyTop = domeRadius
        let bodyBottom = height * 0.15  // In flipped coordinates, this is near the bottom
        
        // Start at bottom left (in non-flipped: top-left of scallops)
        let scallopY = bodyBottom
        path.move(to: CGPoint(x: rect.minX, y: scallopY))
        
        // Wavy bottom with 3 scallops
        let scallops = 3
        let scallopWidth = width / CGFloat(scallops)
        let scallopDepth = height * 0.12
        
        for i in 0..<scallops {
            let startX = rect.minX + CGFloat(i) * scallopWidth
            let midX = startX + scallopWidth / 2
            let endX = startX + scallopWidth
            
            // Curve downward (in non-flipped coords, curves toward 0)
            path.curve(
                to: CGPoint(x: endX, y: scallopY),
                controlPoint1: CGPoint(x: startX + scallopWidth * 0.3, y: scallopY - scallopDepth),
                controlPoint2: CGPoint(x: midX + scallopWidth * 0.2, y: scallopY - scallopDepth)
            )
        }
        
        // Right side going up
        path.line(to: CGPoint(x: rect.maxX, y: height - bodyTop))
        
        // Top dome (arc)
        path.appendArc(
            withCenter: CGPoint(x: rect.midX, y: height - bodyTop),
            radius: domeRadius,
            startAngle: 0,
            endAngle: 180,
            clockwise: false
        )
        
        // Left side going down
        path.line(to: CGPoint(x: rect.minX, y: scallopY))
        
        path.close()
        return path
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
