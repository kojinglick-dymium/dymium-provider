import SwiftUI

/// A minimal ghost shape for the menu bar icon
/// Simple silhouette: rounded dome top, wavy scalloped bottom
struct GhostShape: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        
        let width = rect.width
        let height = rect.height
        
        // The ghost is taller than wide
        // Top half: semicircle dome
        // Bottom half: wavy scalloped edge
        
        let domeHeight = height * 0.55
        let bodyHeight = height * 0.45
        
        // Start at bottom left
        path.move(to: CGPoint(x: 0, y: height))
        
        // Create the wavy bottom with 3 scallops
        let scallops = 3
        let scallopWidth = width / CGFloat(scallops)
        let scallopDepth = bodyHeight * 0.4
        
        for i in 0..<scallops {
            let startX = CGFloat(i) * scallopWidth
            let midX = startX + scallopWidth / 2
            let endX = startX + scallopWidth
            
            // Go up then down for each scallop
            path.addQuadCurve(
                to: CGPoint(x: endX, y: height),
                control: CGPoint(x: midX, y: height - scallopDepth)
            )
        }
        
        // Right side going up
        path.addLine(to: CGPoint(x: width, y: domeHeight))
        
        // Top dome (semicircle)
        path.addArc(
            center: CGPoint(x: width / 2, y: domeHeight),
            radius: width / 2,
            startAngle: .degrees(0),
            endAngle: .degrees(180),
            clockwise: true
        )
        
        // Left side going down
        path.addLine(to: CGPoint(x: 0, y: height))
        
        path.closeSubpath()
        
        return path
    }
}

/// Menu bar status icon using the ghost shape
struct StatusIcon: View {
    let state: TokenState
    
    @State private var isPulsing = false
    
    var body: some View {
        GhostShape()
            .fill(iconColor)
            .frame(width: 16, height: 18)
            .opacity(opacity)
            .animation(pulseAnimation, value: isPulsing)
            .onAppear {
                if state.isAuthenticating {
                    isPulsing = true
                }
            }
            .onChange(of: state) { newState in
                isPulsing = newState.isAuthenticating
            }
    }
    
    private var iconColor: Color {
        switch state {
        case .idle:
            return .gray
        case .authenticating:
            return .yellow
        case .authenticated:
            return .green
        case .failed:
            return .red
        }
    }
    
    private var opacity: Double {
        if state.isAuthenticating {
            return isPulsing ? 1.0 : 0.4
        }
        return 1.0
    }
    
    private var pulseAnimation: Animation? {
        if state.isAuthenticating {
            return .easeInOut(duration: 0.6).repeatForever(autoreverses: true)
        }
        return nil
    }
}
