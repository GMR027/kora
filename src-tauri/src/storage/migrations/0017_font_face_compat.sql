-- Conserva la elección anterior de serif/sans/mono al introducir font_face.
UPDATE reader_settings
SET font_face = 'jakarta'
WHERE font_family = 'sans' AND font_face = 'newsreader';

UPDATE reader_settings
SET font_face = 'mono'
WHERE font_family = 'mono' AND font_face = 'newsreader';
