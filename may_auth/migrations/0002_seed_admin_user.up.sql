-- IMPORTANT: This migration creates a default admin user with password 'changeme'.
-- Operators MUST change this password post-deployment!
INSERT INTO users (username, password_hash, role)
VALUES ('admin', '$2b$12$q/phIyk4Tw0VsbiBWgo29ea2iUQ031y7gpJNhri4P/pljfEKX4YPq', 'admin')
ON CONFLICT (username) DO NOTHING;
