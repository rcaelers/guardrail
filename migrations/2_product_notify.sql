--
-- Trigger to notify on product changes for real-time Valkey cache sync
--

CREATE OR REPLACE FUNCTION core.notify_product_change() RETURNS trigger AS $$
DECLARE
    payload jsonb;
    product_row core.products;
BEGIN
    IF TG_OP = 'DELETE' THEN
        product_row := OLD;
    ELSE
        product_row := NEW;
    END IF;

    payload := jsonb_build_object(
        'op', TG_OP,
        'id', product_row.id,
        'name', product_row.name,
        'accepting_crashes', product_row.accepting_crashes
    );

    -- Include old name on UPDATE so we can remove the stale cache key if renamed
    IF TG_OP = 'UPDATE' AND OLD.name IS DISTINCT FROM NEW.name THEN
        payload := payload || jsonb_build_object('old_name', OLD.name);
    END IF;

    PERFORM pg_notify('product_changed', payload::text);

    RETURN product_row;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION core.notify_product_change() OWNER TO guardrail;

CREATE TRIGGER product_change_trigger
    AFTER INSERT OR UPDATE OR DELETE ON core.products
    FOR EACH ROW
    EXECUTE FUNCTION core.notify_product_change();
