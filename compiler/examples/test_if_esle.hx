package test;

class IfElseTest {
	public static function main():Int {
		var x = 10;
		var y = 5;
		var result = 0;

		if (x > y) {
			result = 100;
		} else {
			result = 200;
		}

		// Should take the if branch (x > y is true)
		// result = 100

		var bonus = 0;
		if (x == 10) {
			bonus = 20;
		}

		// Total: 100 + 20 = 120
		return result + bonus;
	}
}
