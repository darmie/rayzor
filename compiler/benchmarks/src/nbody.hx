// N-Body Benchmark
// Based on https://benchs.haxe.org/nbody/index.html
//
// Tests: Floating-point arithmetic, array iteration, physics calculations

package benchmarks;

class Body {
    public var x:Float;
    public var y:Float;
    public var z:Float;
    public var vx:Float;
    public var vy:Float;
    public var vz:Float;
    public var mass:Float;

    public function new(x:Float, y:Float, z:Float, vx:Float, vy:Float, vz:Float, mass:Float) {
        this.x = x;
        this.y = y;
        this.z = z;
        this.vx = vx;
        this.vy = vy;
        this.vz = vz;
        this.mass = mass;
    }
}

class NBody {
    static inline var PI = 3.141592653589793;
    static inline var SOLAR_MASS = 4.0 * PI * PI;
    static inline var DAYS_PER_YEAR = 365.24;

    static var bodies:Array<Body>;

    public static function main() {
        initBodies();

        // Run simulation
        var n = 500000;
        trace(energy());

        for (i in 0...n) {
            advance(0.01);
        }

        trace(energy());
    }

    static function initBodies() {
        bodies = [
            // Sun
            new Body(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, SOLAR_MASS),
            // Jupiter
            new Body(
                4.84143144246472090e+00,
                -1.16032004402742839e+00,
                -1.03622044471123109e-01,
                1.66007664274403694e-03 * DAYS_PER_YEAR,
                7.69901118419740425e-03 * DAYS_PER_YEAR,
                -6.90460016972063023e-05 * DAYS_PER_YEAR,
                9.54791938424326609e-04 * SOLAR_MASS
            ),
            // Saturn
            new Body(
                8.34336671824457987e+00,
                4.12479856412430479e+00,
                -4.03523417114321381e-01,
                -2.76742510726862411e-03 * DAYS_PER_YEAR,
                4.99852801234917238e-03 * DAYS_PER_YEAR,
                2.30417297573763929e-05 * DAYS_PER_YEAR,
                2.85885980666130812e-04 * SOLAR_MASS
            ),
            // Uranus
            new Body(
                1.28943695621391310e+01,
                -1.51111514016986312e+01,
                -2.23307578892655734e-01,
                2.96460137564761618e-03 * DAYS_PER_YEAR,
                2.37847173959480950e-03 * DAYS_PER_YEAR,
                -2.96589568540237556e-05 * DAYS_PER_YEAR,
                4.36624404335156298e-05 * SOLAR_MASS
            ),
            // Neptune
            new Body(
                1.53796971148509165e+01,
                -2.59193146099879641e+01,
                1.79258772950371181e-01,
                2.68067772490389322e-03 * DAYS_PER_YEAR,
                1.62824170038242295e-03 * DAYS_PER_YEAR,
                -9.51592254519715870e-05 * DAYS_PER_YEAR,
                5.15138902046611451e-05 * SOLAR_MASS
            )
        ];

        // Offset momentum
        var px = 0.0;
        var py = 0.0;
        var pz = 0.0;

        for (b in bodies) {
            px = px + b.vx * b.mass;
            py = py + b.vy * b.mass;
            pz = pz + b.vz * b.mass;
        }

        bodies[0].vx = -px / SOLAR_MASS;
        bodies[0].vy = -py / SOLAR_MASS;
        bodies[0].vz = -pz / SOLAR_MASS;
    }

    static function advance(dt:Float) {
        var len = bodies.length;

        for (i in 0...len) {
            var bi = bodies[i];
            for (j in (i + 1)...len) {
                var bj = bodies[j];
                var dx = bi.x - bj.x;
                var dy = bi.y - bj.y;
                var dz = bi.z - bj.z;

                var dist2 = dx * dx + dy * dy + dz * dz;
                var dist = Math.sqrt(dist2);
                var mag = dt / (dist2 * dist);

                bi.vx = bi.vx - dx * bj.mass * mag;
                bi.vy = bi.vy - dy * bj.mass * mag;
                bi.vz = bi.vz - dz * bj.mass * mag;

                bj.vx = bj.vx + dx * bi.mass * mag;
                bj.vy = bj.vy + dy * bi.mass * mag;
                bj.vz = bj.vz + dz * bi.mass * mag;
            }
        }

        for (b in bodies) {
            b.x = b.x + dt * b.vx;
            b.y = b.y + dt * b.vy;
            b.z = b.z + dt * b.vz;
        }
    }

    static function energy():Float {
        var e = 0.0;
        var len = bodies.length;

        for (i in 0...len) {
            var bi = bodies[i];
            e = e + 0.5 * bi.mass * (bi.vx * bi.vx + bi.vy * bi.vy + bi.vz * bi.vz);

            for (j in (i + 1)...len) {
                var bj = bodies[j];
                var dx = bi.x - bj.x;
                var dy = bi.y - bj.y;
                var dz = bi.z - bj.z;
                var dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
                e = e - (bi.mass * bj.mass) / dist;
            }
        }

        return e;
    }
}
