extern crate statrs;
extern crate plotters;

use statrs::distribution::{Normal, ContinuousCDF};
use std::f64;
use std::vec::Vec;
use plotters::prelude::*;  // For chart generation

struct OptimalExerciseBinTree {
    spot_price: f64,
    payoff: Box<dyn Fn(f64, f64) -> f64>,
    expiry: f64,
    rate: f64,
    vol: f64,
    num_steps: usize,
}

impl OptimalExerciseBinTree {
    fn dt(&self) -> f64 {
        self.expiry / self.num_steps as f64
    }

    fn state_price(&self, i: usize, j: usize) -> f64 {
        self.spot_price
            * ((2 * j as i64 - i as i64) as f64 * self.vol * self.dt().sqrt()).exp()
    }

    fn get_opt_vf_and_policy(&self) -> (Vec<Vec<f64>>, Vec<Vec<bool>>) {
        let dt = self.dt();
        let gamma = (-self.rate * dt).exp();
        let up_factor = (self.vol * dt.sqrt()).exp();
        let exp_rate_dt = (self.rate * dt).exp();
        let up_prob = (exp_rate_dt * up_factor - 1.0) / (up_factor * up_factor - 1.0);

        let mut vf_seq: Vec<Vec<f64>> = Vec::with_capacity(self.num_steps + 1);
        let mut policy_seq: Vec<Vec<bool>> = Vec::with_capacity(self.num_steps + 1);

        // Initialize v_prev
        let mut v_prev = vec![0.0; self.num_steps + 2];

        for i in (0..=self.num_steps).rev() {
            let mut v_curr = vec![0.0; i + 1];
            let mut policy = vec![false; i + 1];

            for j in 0..=i {
                let s = self.state_price(i, j);
                let exercise_reward = (self.payoff)(i as f64 * dt, s);
                let v_exercise = exercise_reward;
                let v_continue = if i == self.num_steps {
                    0.0
                } else {
                    gamma * (up_prob * v_prev[j + 1] + (1.0 - up_prob) * v_prev[j])
                };

                if v_exercise >= v_continue {
                    v_curr[j] = v_exercise;
                    policy[j] = true;
                } else {
                    v_curr[j] = v_continue;
                    policy[j] = false;
                }
            }

            vf_seq.push(v_curr.clone());
            policy_seq.push(policy.clone());
            // Prepare v_prev for next iteration
            v_prev[0..=i].copy_from_slice(&v_curr[0..=i]);
        }

        vf_seq.reverse();
        policy_seq.reverse();

        (vf_seq, policy_seq)
    }

    fn option_exercise_boundary(
        &self,
        policy_seq: &Vec<Vec<bool>>,
        is_call: bool,
    ) -> Vec<(f64, f64)> {
        let dt = self.dt();
        let mut ex_boundary = Vec::new();
        for (i, policy) in policy_seq.iter().enumerate() {
            let mut ex_points = Vec::new();
            for (j, &action) in policy.iter().enumerate() {
                if action {
                    let s = self.state_price(i, j);
                    let payoff = (self.payoff)(i as f64 * dt, s);
                    if payoff > 0.0 {
                        ex_points.push(j);
                    }
                }
            }
            if !ex_points.is_empty() {
                let boundary_j = if is_call {
                    *ex_points.iter().min().unwrap()
                } else {
                    *ex_points.iter().max().unwrap()
                };
                let boundary_s = self.state_price(i, boundary_j);
                ex_boundary.push((i as f64 * dt, boundary_s));
            }
        }
        ex_boundary
    }

    fn european_price(&self, is_call: bool, strike: f64) -> f64 {
        let sigma_sqrt = self.vol * self.expiry.sqrt();
        let d1 = ((self.spot_price / strike).ln()
            + (self.rate + self.vol * self.vol / 2.0) * self.expiry)
            / sigma_sqrt;
        let d2 = d1 - sigma_sqrt;
        let norm = Normal::new(0.0, 1.0).unwrap();
        if is_call {
            self.spot_price * norm.cdf(d1)
                - strike * (-self.rate * self.expiry).exp() * norm.cdf(d2)
        } else {
            strike * (-self.rate * self.expiry).exp() * norm.cdf(-d2)
                - self.spot_price * norm.cdf(-d1)
        }
    }
}

// Function to plot exercise boundary chart
fn plot_exercise_boundary(ex_boundary: &Vec<(f64, f64)>, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new("exercise_boundary.png", (1080, 720)).into_drawing_area();
    root.fill(&WHITE)?;

    let (x_vals, y_vals): (Vec<f64>, Vec<f64>) = ex_boundary.iter().cloned().unzip();
    let mut chart = ChartBuilder::on(&root)
        .caption(title, ("sans-serif", 50).into_font())
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0f64..x_vals.iter().cloned().fold(0./0., f64::max), 
                            0f64..y_vals.iter().cloned().fold(0./0., f64::max))?;

    chart.configure_mesh().draw()?;

    chart.draw_series(LineSeries::new(
        x_vals.into_iter().zip(y_vals.into_iter()),
        &RED,
    ))?;

    root.present()?;
    Ok(())
}

// Function to plot option price evolution over time and asset prices
fn plot_option_price_evolution(vf_seq: &Vec<Vec<f64>>, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new("option_price_evolution.png", (1080, 720)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(title, ("sans-serif", 50).into_font())
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0f64..(vf_seq.len() as f64), 
                            0f64..vf_seq.iter().flatten().cloned().fold(0./0., f64::max))?;

    chart.configure_mesh().draw()?;

    for (i, vf) in vf_seq.iter().enumerate() {
        let vf_prices: Vec<(f64, f64)> = vf.iter().enumerate().map(|(j, &v)| (i as f64, v)).collect();
        chart.draw_series(LineSeries::new(vf_prices, &BLUE))?;
    }

    root.present()?;
    Ok(())
}

fn main() {
    let spot_price_val = 100.0;
    let strike = 100.0;
    let is_call = false;
    let expiry_val = 1.0;
    let rate_val = 0.05;
    let vol_val = 0.25;
    let num_steps_val = 300;

    let payoff: Box<dyn Fn(f64, f64) -> f64> = Box::new(move |_t: f64, s: f64| {
        if is_call {
            f64::max(s - strike, 0.0)
        } else {
            f64::max(strike - s, 0.0)
        }
    });

    let opt_ex_bin_tree = OptimalExerciseBinTree {
        spot_price: spot_price_val,
        payoff: payoff,
        expiry: expiry_val,
        rate: rate_val,
        vol: vol_val,
        num_steps: num_steps_val,
    };

    let (vf_seq, policy_seq) = opt_ex_bin_tree.get_opt_vf_and_policy();

    let european = opt_ex_bin_tree.european_price(is_call, strike);
    println!("European Price = {:.3}", european);

    let am_price = vf_seq[0][0];
    println!("American Price = {:.3}", am_price);

    // Optionally, print the exercise boundary
    let ex_boundary = opt_ex_bin_tree.option_exercise_boundary(&policy_seq, is_call);

    println!("\nExercise Boundary Points:");
    for (t, s) in &ex_boundary {
        println!("Time: {:.3}, Exercise Boundary Price: {:.3}", t, s);
    }

    // Generate the plot for the exercise boundary
    plot_exercise_boundary(&ex_boundary, "American Option Exercise Boundary").expect("Failed to create chart");

    // Plot option price evolution
    plot_option_price_evolution(&vf_seq, "Option Price Evolution").expect("Failed to create chart");
}
