import UIKit

// Declare the Rust entry point (#[no_mangle] pub extern "C" fn quad_main())
@_silgen_name("quad_main")
func quad_main()

class ViewController: UIViewController {
    private var started = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard !started else { return }
        started = true
        // quad_main() blocks forever running the game loop,
        // so it must run on a background thread.
        DispatchQueue.global(qos: .userInteractive).async {
            quad_main()
        }
    }

    // Force landscape orientation for the rhythm game
    override var shouldAutorotate: Bool { true }
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask {
        .landscape
    }
    override var preferredInterfaceOrientationForPresentation: UIInterfaceOrientation {
        .landscapeRight
    }
    override var prefersStatusBarHidden: Bool { true }
}
