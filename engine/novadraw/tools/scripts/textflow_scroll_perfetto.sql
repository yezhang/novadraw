-- Run on a warm official Trace Processor session.
-- Export each SELECT independently as CSV. The record payload is SDK debug JSON.
CREATE OR REPLACE PERFETTO TABLE scroll_self AS
WITH children AS (
  SELECT parent_id, sum(dur) AS ns
  FROM slice WHERE dur >= 0 GROUP BY parent_id
)
SELECT s.*, s.dur - coalesce(c.ns, 0) AS self_ns
FROM slice s LEFT JOIN children c ON c.parent_id = s.id;

CREATE OR REPLACE PERFETTO TABLE scroll_roots AS
SELECT * FROM slice
WHERE dur >= 0 AND name IN (
  'host-operation', 'host-frame', 'host-input-dispatch'
);

-- Health: no unfinished scopes, negative exclusive duration or parser loss.
SELECT name, severity, value FROM stats WHERE value != 0;
SELECT count(*) AS unfinished FROM slice WHERE dur < 0;
SELECT count(*) AS negative_self FROM scroll_self WHERE self_ns < 0;

-- Distinguish measurement builds from preparation builds by immediate parent.
SELECT r.id AS root_id, p.name AS parent_name, count(*) AS calls, sum(s.dur) AS ns
FROM scroll_roots r JOIN slice s
  ON s.track_id = r.track_id AND s.ts >= r.ts AND s.ts + s.dur <= r.ts + r.dur
JOIN slice p ON p.id = s.parent_id
WHERE s.name = 'flow-build' AND s.dur >= 0
GROUP BY r.id, p.name;

-- Record parent identifies operation/frame/input scope, without timestamp guesses.
-- Hex preserves embedded JSON quotes in the processor's CSV output.
SELECT s.id, s.parent_id, s.ts,
       hex(EXTRACT_ARG(s.arg_set_id, 'debug.record')) AS record
FROM slice s WHERE s.name = 'host-record' ORDER BY s.ts;

SELECT id, parent_id, track_id, ts, dur, name FROM scroll_roots ORDER BY ts;

-- Every nested synchronous stage belongs to the corresponding root on its track.
-- Input includes its nested operation. Never add those inclusive totals together.
SELECT r.id AS root_id, r.name AS root_name, s.name,
       count(*) AS calls, sum(s.dur) AS inclusive_ns,
       sum(s.self_ns) AS exclusive_ns
FROM scroll_roots r JOIN scroll_self s
  ON s.track_id = r.track_id AND s.ts >= r.ts
 AND s.ts < r.ts + r.dur AND s.ts + s.dur <= r.ts + r.dur
GROUP BY r.id, r.name, s.name;
