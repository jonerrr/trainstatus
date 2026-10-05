use super::*;

    #[test]
    fn chooses_headsign_with_lex_tie_breaker() {
        let mut counts = HashMap::new();
        counts.insert("Downtown".to_string(), 3);
        counts.insert("Uptown".to_string(), 3);

        let winner = choose_headsign(counts);
        assert_eq!(winner, "Downtown");
    }

    #[test]
    fn opposite_stop_matching_is_bidirectional() {
        let mut stop_geom_map = HashMap::new();
        stop_geom_map.insert("A0".to_string(), Point::new(0.0, 0.0));
        stop_geom_map.insert("A1".to_string(), Point::new(100.0, 0.0));
        stop_geom_map.insert("B0".to_string(), Point::new(2.0, 0.0));
        stop_geom_map.insert("B1".to_string(), Point::new(98.0, 0.0));

        let dir0 = vec!["A0".to_string(), "A1".to_string()];
        let dir1 = vec!["B0".to_string(), "B1".to_string()];

        let result = compute_opposite_stops(&dir0, &dir1, &stop_geom_map, 10.0);

        assert_eq!(result.get("A0"), Some(&"B0".to_string()));
        assert_eq!(result.get("A1"), Some(&"B1".to_string()));
        assert_eq!(result.get("B0"), Some(&"A0".to_string()));
        assert_eq!(result.get("B1"), Some(&"A1".to_string()));
    }

    #[test]
    fn shape_to_linestring_sorts_and_dedupes_points() {
        let shape = vec![
            Shape {
                id: "s1".to_string(),
                sequence: 2,
                latitude: 0.0,
                longitude: 1.0,
                ..Default::default()
            },
            Shape {
                id: "s1".to_string(),
                sequence: 1,
                latitude: 0.0,
                longitude: 0.0,
                ..Default::default()
            },
            Shape {
                id: "s1".to_string(),
                sequence: 3,
                latitude: 0.0,
                longitude: 1.0,
                ..Default::default()
            },
        ];

        let line = shape_points_to_linestring(&shape);
        assert_eq!(line.0.len(), 2);
        assert_eq!(line.0[0].x, 0.0);
        assert_eq!(line.0[1].x, 1.0);
    }
