-- Lao comment-to-order trigger words: ເອົາ (take), ຮັບ (get), ສັ່ງ (order), ຈອງ (reserve).

ALTER TABLE shops ALTER COLUMN social_triggers
    SET DEFAULT ARRAY['cf','f','เอา','รับ','สั่ง','order','buy','ເອົາ','ຮັບ','ສັ່ງ','ຈອງ'];

-- Append the Lao words to existing shops, skipping any a shop already has.
UPDATE shops s
   SET social_triggers = array_cat(
           s.social_triggers,
           ARRAY(SELECT w FROM unnest(ARRAY['ເອົາ','ຮັບ','ສັ່ງ','ຈອງ']::text[]) WITH ORDINALITY AS t(w, i)
                  WHERE NOT (w = ANY (s.social_triggers))
                  ORDER BY i))
 WHERE NOT (s.social_triggers @> ARRAY['ເອົາ','ຮັບ','ສັ່ງ','ຈອງ']::text[]);
