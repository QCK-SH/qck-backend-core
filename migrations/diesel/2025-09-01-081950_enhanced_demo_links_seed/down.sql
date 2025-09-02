-- Remove enhanced demo links added by this migration
-- This removes all links created by the enhanced seed migration
-- But keeps the original demo links from the first migration

-- Remove additional free tier links (those with specific short_codes from enhancement)
DELETE FROM links WHERE user_id = 'f1111111-1111-1111-1111-111111111111' 
AND short_code LIKE 'free%' AND short_code NOT LIKE 'free00%';

-- Remove additional pro tier links (pro011-pro100)
DELETE FROM links WHERE user_id = 'f2222222-2222-2222-2222-222222222222' 
AND short_code LIKE 'pro%' AND short_code NOT LIKE 'pro00%';

-- Remove additional business tier links (biz016-biz200)  
DELETE FROM links WHERE user_id = 'f3333333-3333-3333-3333-333333333333'
AND short_code LIKE 'biz%';

-- Remove additional enterprise tier links (ent021-ent500)
DELETE FROM links WHERE user_id = 'f4444444-4444-4444-4444-444444444444'
AND short_code LIKE 'ent%';
