use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, Attribute, Cell, CellAlignment, Color, Row, Table,
};
use forge::result::TestOutcome;

/// A simple summary reporter that prints the test results in a table.
pub struct TestSummaryReporter {
    /// The test summary table.
    pub(crate) table: Table,
    pub(crate) is_detailed: bool,
}

impl TestSummaryReporter {
    pub(crate) fn new(is_detailed: bool) -> Self {
        let mut table = Table::new();
        table.apply_modifier(UTF8_ROUND_CORNERS);
        let mut row = Row::from(vec![
            Cell::new("Test Suite")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold),
            Cell::new("Passed")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)
                .fg(Color::Green),
            Cell::new("Failed")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)
                .fg(Color::Red),
            Cell::new("Skipped")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)
                .fg(Color::Yellow),
        ]);
        if is_detailed {
            row.add_cell(
                Cell::new("File Path")
                    .set_alignment(CellAlignment::Center)
                    .add_attribute(Attribute::Bold),
            );
            row.add_cell(
                Cell::new("Duration")
                    .set_alignment(CellAlignment::Center)
                    .add_attribute(Attribute::Bold),
            );
        }
        table.set_header(row);

        Self { table, is_detailed }
    }

    pub(crate) fn print_summary(&mut self, outcome: &TestOutcome) {
        // Traverse the test_results vector and build the table
        for (contract, suite) in &outcome.results {
            let mut row = Row::new();
            let (suite_path, suite_name) = contract.split_once(':').unwrap();

            let passed = suite.successes().count();
            let mut passed_cell = Cell::new(passed).set_alignment(CellAlignment::Center);

            let failed = suite.failures().count();
            let mut failed_cell = Cell::new(failed).set_alignment(CellAlignment::Center);

            let skipped = suite.skips().count();
            let mut skipped_cell = Cell::new(skipped).set_alignment(CellAlignment::Center);

            row.add_cell(Cell::new(suite_name));

            if passed > 0 {
                passed_cell = passed_cell.fg(Color::Green);
            }
            row.add_cell(passed_cell);

            if failed > 0 {
                failed_cell = failed_cell.fg(Color::Red);
            }
            row.add_cell(failed_cell);

            if skipped > 0 {
                skipped_cell = skipped_cell.fg(Color::Yellow);
            }
            row.add_cell(skipped_cell);

            if self.is_detailed {
                row.add_cell(Cell::new(suite_path));
                row.add_cell(Cell::new(format!("{:.2?}", suite.duration).to_string()));
            }

            self.table.add_row(row);
        }

        println!("\n{}", self.table);
    }
}
