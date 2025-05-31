-- Recreate the ENUM type
CREATE TYPE status AS ENUM (
    -- train
    'none',
    'incoming',
    'at_stop',
    'in_transit_to',
    -- bus
    'spooking',
    'layover',
    'no_progress'
);

-- Alter the table to change the column type back to the ENUM
-- The USING clause is necessary to cast the existing VARCHAR data to the ENUM type.
ALTER TABLE position
ALTER COLUMN status TYPE status
USING status::status;