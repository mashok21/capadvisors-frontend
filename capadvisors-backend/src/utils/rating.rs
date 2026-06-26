const GL2_SCALE: f64 = 173.7178;
const DEFAULT_TAU: f64 = 0.5;
const CONVERGENCE_TOLERANCE: f64 = 0.000001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glicko2Rating {
    pub rating: f64,
    pub rating_deviation: f64,
    pub volatility: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TournamentResult {
    pub rating: Glicko2Rating,
    pub placement: u32,
    pub is_target: bool,
}

#[derive(Debug, Clone, Copy)]
struct MatchResult {
    opponent: Glicko2Rating,
    score: f64,
}

pub fn calculate_tournament_performance(
    current_rating: &Glicko2Rating,
    standings: Vec<TournamentResult>,
) -> Glicko2Rating {
    let Some(target) = standings.iter().find(|row| row.is_target) else {
        return *current_rating;
    };

    let matches: Vec<MatchResult> = standings
        .iter()
        .filter(|row| !row.is_target)
        .filter_map(|row| {
            let score = if row.placement > target.placement {
                1.0
            } else if row.placement < target.placement {
                0.0
            } else {
                return None;
            };

            Some(MatchResult {
                opponent: row.rating,
                score,
            })
        })
        .collect();

    calculate_rating_period(current_rating, &matches, DEFAULT_TAU)
}

fn calculate_rating_period(
    current_rating: &Glicko2Rating,
    matches: &[MatchResult],
    tau: f64,
) -> Glicko2Rating {
    let mu = to_mu(current_rating.rating);
    let phi = to_phi(current_rating.rating_deviation);
    let sigma = current_rating.volatility;

    if matches.is_empty() {
        let phi_star = (phi.powi(2) + sigma.powi(2)).sqrt();
        return Glicko2Rating {
            rating: current_rating.rating,
            rating_deviation: from_phi(phi_star),
            volatility: sigma,
        };
    }

    let v_inv: f64 = matches
        .iter()
        .map(|result| {
            let opponent_mu = to_mu(result.opponent.rating);
            let opponent_phi = to_phi(result.opponent.rating_deviation);
            let g = g(opponent_phi);
            let e = expected_score(mu, opponent_mu, opponent_phi);
            g.powi(2) * e * (1.0 - e)
        })
        .sum();
    let v = 1.0 / v_inv;

    let score_delta_sum: f64 = matches
        .iter()
        .map(|result| {
            let opponent_mu = to_mu(result.opponent.rating);
            let opponent_phi = to_phi(result.opponent.rating_deviation);
            let g = g(opponent_phi);
            let e = expected_score(mu, opponent_mu, opponent_phi);
            g * (result.score - e)
        })
        .sum();
    let delta = v * score_delta_sum;

    let sigma_prime = updated_volatility(phi, sigma, delta, v, tau);
    let phi_star = (phi.powi(2) + sigma_prime.powi(2)).sqrt();
    let phi_prime = 1.0 / ((1.0 / phi_star.powi(2)) + (1.0 / v)).sqrt();
    let mu_prime = mu + phi_prime.powi(2) * score_delta_sum;

    Glicko2Rating {
        rating: from_mu(mu_prime),
        rating_deviation: from_phi(phi_prime),
        volatility: sigma_prime,
    }
}

fn updated_volatility(phi: f64, sigma: f64, delta: f64, v: f64, tau: f64) -> f64 {
    let a = sigma.powi(2).ln();
    let mut a_curr = a;
    let mut b_curr = if delta.powi(2) > phi.powi(2) + v {
        (delta.powi(2) - phi.powi(2) - v).ln()
    } else {
        let mut k = 1.0;
        while volatility_objective(a - k * tau, a, phi, delta, v, tau) < 0.0 {
            k += 1.0;
        }
        a - k * tau
    };

    let mut f_a = volatility_objective(a_curr, a, phi, delta, v, tau);
    let mut f_b = volatility_objective(b_curr, a, phi, delta, v, tau);

    while (b_curr - a_curr).abs() > CONVERGENCE_TOLERANCE {
        let c = a_curr + (a_curr - b_curr) * f_a / (f_b - f_a);
        let f_c = volatility_objective(c, a, phi, delta, v, tau);

        if f_c * f_b <= 0.0 {
            a_curr = b_curr;
            f_a = f_b;
        } else {
            f_a /= 2.0;
        }

        b_curr = c;
        f_b = f_c;
    }

    (a_curr / 2.0).exp()
}

fn volatility_objective(x: f64, a: f64, phi: f64, delta: f64, v: f64, tau: f64) -> f64 {
    let exp_x = x.exp();
    let denominator = phi.powi(2) + v + exp_x;
    (exp_x * (delta.powi(2) - denominator) / (2.0 * denominator.powi(2))) - ((x - a) / tau.powi(2))
}

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + (3.0 * phi.powi(2) / std::f64::consts::PI.powi(2))).sqrt()
}

fn expected_score(mu: f64, opponent_mu: f64, opponent_phi: f64) -> f64 {
    1.0 / (1.0 + (-g(opponent_phi) * (mu - opponent_mu)).exp())
}

fn to_mu(rating: f64) -> f64 {
    (rating - 1500.0) / GL2_SCALE
}

fn from_mu(mu: f64) -> f64 {
    (mu * GL2_SCALE) + 1500.0
}

fn to_phi(rating_deviation: f64) -> f64 {
    rating_deviation / GL2_SCALE
}

fn from_phi(phi: f64) -> f64 {
    phi * GL2_SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rating(rating: f64, rating_deviation: f64, volatility: f64) -> Glicko2Rating {
        Glicko2Rating {
            rating,
            rating_deviation,
            volatility,
        }
    }

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {actual} to be within {tolerance} of {expected}"
        );
    }

    #[test]
    fn matches_glickman_rating_period_benchmark() {
        let current = rating(1500.0, 200.0, 0.06);
        let standings = vec![
            TournamentResult {
                rating: current,
                placement: 3,
                is_target: true,
            },
            TournamentResult {
                rating: rating(1400.0, 30.0, 0.06),
                placement: 4,
                is_target: false,
            },
            TournamentResult {
                rating: rating(1550.0, 100.0, 0.06),
                placement: 2,
                is_target: false,
            },
            TournamentResult {
                rating: rating(1700.0, 300.0, 0.06),
                placement: 1,
                is_target: false,
            },
        ];

        let updated = calculate_tournament_performance(&current, standings);

        assert_close(updated.rating, 1464.06, 0.01);
        assert_close(updated.rating_deviation, 151.52, 0.01);
        assert_close(updated.volatility, 0.05999, 0.00001);
    }

    #[test]
    fn scores_all_lower_placements_as_wins() {
        let current = rating(1500.0, 350.0, 0.06);
        let standings = vec![
            TournamentResult {
                rating: current,
                placement: 1,
                is_target: true,
            },
            TournamentResult {
                rating: rating(1600.0, 80.0, 0.06),
                placement: 2,
                is_target: false,
            },
            TournamentResult {
                rating: rating(1550.0, 100.0, 0.06),
                placement: 3,
                is_target: false,
            },
            TournamentResult {
                rating: rating(1450.0, 60.0, 0.06),
                placement: 4,
                is_target: false,
            },
        ];

        let updated = calculate_tournament_performance(&current, standings);

        assert!(updated.rating > current.rating);
        assert!(updated.rating_deviation < current.rating_deviation);
    }

    #[test]
    fn scores_all_higher_placements_as_losses() {
        let current = rating(1500.0, 350.0, 0.06);
        let standings = vec![
            TournamentResult {
                rating: rating(1600.0, 80.0, 0.06),
                placement: 1,
                is_target: false,
            },
            TournamentResult {
                rating: rating(1550.0, 100.0, 0.06),
                placement: 2,
                is_target: false,
            },
            TournamentResult {
                rating: rating(1450.0, 60.0, 0.06),
                placement: 3,
                is_target: false,
            },
            TournamentResult {
                rating: current,
                placement: 4,
                is_target: true,
            },
        ];

        let updated = calculate_tournament_performance(&current, standings);

        assert!(updated.rating < current.rating);
        assert!(updated.rating_deviation < current.rating_deviation);
    }
}
