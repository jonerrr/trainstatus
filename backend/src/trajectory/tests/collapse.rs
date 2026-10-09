use crate::trajectory::builder::collapse_backtracking_knots_with_stats;
use crate::trajectory::types::TrajectoryKnot;

#[test]
fn collapse_drops_backtrack_keeps_dwell() {
    let knots = vec![
        TrajectoryKnot::new(0.0, 0.0),
        TrajectoryKnot::new(10.0, 100.0),
        TrajectoryKnot::new(40.0, 100.0),
        TrajectoryKnot::new(50.0, 50.0),
        TrajectoryKnot::new(60.0, 120.0),
    ];
    let (out, _removed) = collapse_backtracking_knots_with_stats(&knots);
    assert_eq!(out.len(), 4);
    assert_eq!(out[0].t_event, 0.0);
    assert_eq!(out[1].t_event, 10.0);
    assert_eq!(out[2].t_event, 40.0); // Dwell knot preserved!
    assert_eq!(out[3].t_event, 60.0);
    for i in 1..out.len() {
        assert!(out[i].s_m >= out[i - 1].s_m);
    }
}
