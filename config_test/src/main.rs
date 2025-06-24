use arm_bot_lib::{ArmConfig, SimulationConfig, ForwardKinematics};
use clap::Parser;
use eyre::Result;
use tracing::info;

#[derive(Parser)]
#[command(name = "config_test")]
#[command(about = "Test robotic arm configuration files")]
struct Cli {
    #[arg(short, long, default_value = "config/arm_6dof.toml")]
    config: String,
}

fn main() -> Result<()> {
    let _guard = init_tracing();
    let cli = Cli::parse();

    info!("Testing configuration file: {}", cli.config);
    test_config(&cli.config)?;

    Ok(())
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        )
        .finish();

    tracing::subscriber::set_default(subscriber)
}

fn test_config(config_path: &str) -> Result<()> {
    println!("Testing configuration file: {}", config_path);

    // Test arm configuration
    match ArmConfig::load_from_file(config_path) {
        Ok(config) => {
            println!("✓ Arm configuration loaded successfully");
            println!("  Name: {}", config.name);
            println!("  DOF: {}", config.dof);
            println!("  Joint limits: {} entries", config.joint_limits.len());
            println!("  DH parameters: {} entries", config.kinematics.dh_parameters.len());

            match config.validate() {
                Ok(_) => println!("✓ Configuration validation passed"),
                Err(e) => {
                    println!("✗ Configuration validation failed: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to load arm configuration: {}", e);
            return Err(e);
        }
    }

    // Test simulation configuration
    match SimulationConfig::load_from_file("config/simulation.toml") {
        Ok(sim_config) => {
            println!("✓ Simulation configuration loaded successfully");
            println!("  Unity port: {}", sim_config.unity_websocket_port);
            println!("  Update rate: {} Hz", sim_config.update_rate_hz);
        }
        Err(e) => {
            println!("⚠ Warning: Could not load simulation config: {}", e);
            println!("  This is okay if you're only testing arm config");
        }
    }

    // Test forward kinematics
    let config = ArmConfig::load_from_file(config_path)?;
    match ForwardKinematics::new(&config) {
        Ok(fk) => {
            println!("✓ Forward kinematics initialized successfully");

            // Test with zero angles
            let zero_angles = vec![0.0; config.dof];
            match fk.compute_end_effector_pose(&zero_angles) {
                Ok(pose) => {
                    println!("✓ Forward kinematics test passed");
                    println!("  End effector pose at zero angles: [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
                             pose[0], pose[1], pose[2], pose[3], pose[4], pose[5]);
                }
                Err(e) => {
                    println!("✗ Forward kinematics test failed: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to initialize forward kinematics: {}", e);
            return Err(e);
        }
    }

    println!("\n🎉 All configuration tests passed!");
    Ok(())
}