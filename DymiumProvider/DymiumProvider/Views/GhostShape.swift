import SwiftUI

/// Animated ghost with rotating gradient for "in progress" state
struct AnimatedGradientGhost: View {
    @State private var rotation: Double = 0
    
    var body: some View {
        ZStack {
            // Subtle glow for contrast
            GhostShape()
                .stroke(Color.white.opacity(0.3), lineWidth: 3)
                .blur(radius: 1)
            
            // Main ghost with animated gradient
            // Use multiple color stops for smooth blending
            GhostShape()
                .stroke(
                    AngularGradient(
                        stops: [
                            .init(color: .dymiumAccent, location: 0.0),
                            .init(color: .dymiumPrimary, location: 0.25),
                            .init(color: .dymiumAccent, location: 0.5),
                            .init(color: .dymiumPrimary, location: 0.75),
                            .init(color: .dymiumAccent, location: 1.0)
                        ],
                        center: .center,
                        angle: .degrees(rotation)
                    ),
                    lineWidth: 1.5
                )
        }
        .onAppear {
            withAnimation(.linear(duration: 2.0).repeatForever(autoreverses: false)) {
                rotation = 360
            }
        }
    }
}

/// Static gradient ghost for authenticated state
struct GradientGhost: View {
    var body: some View {
        ZStack {
            // Subtle glow for contrast against dark backgrounds
            GhostShape()
                .stroke(Color.white.opacity(0.3), lineWidth: 3)
                .blur(radius: 1)
            
            // Main ghost with smooth gradient
            // Use multiple stops to create a smoother blend around the shape
            GhostShape()
                .stroke(
                    LinearGradient(
                        stops: [
                            .init(color: .dymiumAccent, location: 0.0),
                            .init(color: Color(red: 0x6A/255, green: 0x5B/255, blue: 0xff/255), location: 0.5),  // Midpoint blend
                            .init(color: .dymiumPrimary, location: 1.0)
                        ],
                        startPoint: .top,
                        endPoint: .bottom
                    ),
                    lineWidth: 1.5
                )
        }
    }
}

/// A minimal ghost shape for the menu bar icon
/// Simple silhouette: rounded dome top, wavy scalloped bottom
/// Designed to be narrower and work well as an outline (stroked)
struct GhostShape: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        
        let width = rect.width
        let height = rect.height
        
        // The ghost is taller than wide (approximately 3:4 aspect ratio)
        // Top: rounded dome
        // Body: straight sides
        // Bottom: wavy scalloped edge with 3 bumps
        
        let domeRadius = width / 2
        let domeHeight = domeRadius  // The dome takes up width/2 of height
        let bodyTop = domeHeight
        let bodyBottom = height * 0.85  // Leave room for scallops
        
        // Start at bottom left, just before the first scallop
        path.move(to: CGPoint(x: 0, y: bodyBottom))
        
        // Create the wavy bottom with 3 scallops (bumps hanging down)
        let scallops = 3
        let scallopWidth = width / CGFloat(scallops)
        let scallopDepth = height * 0.15  // How far scallops hang down
        
        for i in 0..<scallops {
            let startX = CGFloat(i) * scallopWidth
            let midX = startX + scallopWidth / 2
            let endX = startX + scallopWidth
            
            // Arc downward for each scallop
            path.addQuadCurve(
                to: CGPoint(x: endX, y: bodyBottom),
                control: CGPoint(x: midX, y: bodyBottom + scallopDepth)
            )
        }
        
        // Right side going up
        path.addLine(to: CGPoint(x: width, y: bodyTop))
        
        // Top dome (semicircle arc)
        path.addArc(
            center: CGPoint(x: width / 2, y: bodyTop),
            radius: domeRadius,
            startAngle: .degrees(0),
            endAngle: .degrees(180),
            clockwise: true
        )
        
        // Left side going down
        path.addLine(to: CGPoint(x: 0, y: bodyBottom))
        
        path.closeSubpath()
        
        return path
    }
}

/// Menu bar status icon using the ghost shape with animated gradient
struct StatusIcon: View {
    let state: TokenState
    
    @State private var gradientRotation: Double = 0
    
    var body: some View {
        ZStack {
            switch state {
            case .idle, .failed:
                // Gray outline for idle/not authenticated
                GhostShape()
                    .stroke(Color.gray, lineWidth: 1.5)
                    .frame(width: 14, height: 20)
                
            case .authenticating:
                // Morphing/rotating gradient for authenticating
                AnimatedGradientGhost()
                    .frame(width: 14, height: 20)
                
            case .authenticated:
                // Static accent-primary gradient for success
                GradientGhost()
                    .frame(width: 14, height: 20)
            }
        }
    }
}
