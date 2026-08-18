-- Monica and vCard exports can represent birthdays without a known year.
-- Earlier Pandan imports retained these values in a generated note marker.
UPDATE contacts
SET birthday = '--' || substr(
    notes,
    instr(notes, 'Birthday (year unknown): ') + 25,
    5
)
WHERE birthday IS NULL
  AND instr(notes, 'Birthday (year unknown): ') > 0
  AND substr(notes, instr(notes, 'Birthday (year unknown): ') + 27, 1) = '-'
  AND CAST(substr(notes, instr(notes, 'Birthday (year unknown): ') + 25, 2) AS INTEGER)
      BETWEEN 1 AND 12
  AND CAST(substr(notes, instr(notes, 'Birthday (year unknown): ') + 28, 2) AS INTEGER)
      BETWEEN 1 AND 31;
