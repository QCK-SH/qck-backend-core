-- Enhanced Demo Links Seed Data for Comprehensive Testing
-- This creates ALL demo links for realistic testing scenarios
-- Free: 50 links, Pro: 100 links, Business: 200 links, Enterprise: 500 links
-- PROTECTED: This migration is automatically skipped in production environment

-- Basic links for all tier users (moved from seed_demo_users migration)
-- Free user: 5 links
INSERT INTO links (id, user_id, short_code, original_url, title, is_active, created_at, updated_at)
VALUES
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'free001', 'https://example.com/free/page1', 'Free Link 1', true, NOW() - INTERVAL '5 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'free002', 'https://example.com/free/page2', 'Free Link 2', true, NOW() - INTERVAL '4 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'free003', 'https://example.com/free/page3', 'Free Link 3', true, NOW() - INTERVAL '3 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'free004', 'https://example.com/free/page4', 'Free Link 4', true, NOW() - INTERVAL '2 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'free005', 'https://example.com/free/page5', 'Free Link 5', true, NOW() - INTERVAL '1 day', NOW());

-- Pro user: 10 links
INSERT INTO links (id, user_id, short_code, original_url, title, is_active, created_at, updated_at)
VALUES
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro001', 'https://example.com/pro/page1', 'Pro Link 1', true, NOW() - INTERVAL '10 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro002', 'https://example.com/pro/page2', 'Pro Link 2', true, NOW() - INTERVAL '9 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro003', 'https://example.com/pro/page3', 'Pro Link 3', true, NOW() - INTERVAL '8 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro004', 'https://example.com/pro/page4', 'Pro Link 4', true, NOW() - INTERVAL '7 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro005', 'https://example.com/pro/page5', 'Pro Link 5', true, NOW() - INTERVAL '6 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro006', 'https://example.com/pro/page6', 'Pro Link 6', true, NOW() - INTERVAL '5 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro007', 'https://example.com/pro/page7', 'Pro Link 7', true, NOW() - INTERVAL '4 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro008', 'https://example.com/pro/page8', 'Pro Link 8', true, NOW() - INTERVAL '3 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro009', 'https://example.com/pro/page9', 'Pro Link 9', true, NOW() - INTERVAL '2 days', NOW()),
    (gen_random_uuid(), 'f2222222-2222-2222-2222-222222222222', 'pro010', 'https://example.com/pro/page10', 'Pro Link 10', true, NOW() - INTERVAL '1 day', NOW());

-- Business user: 15 links
INSERT INTO links (id, user_id, short_code, original_url, title, is_active, created_at, updated_at)
VALUES
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz001', 'https://example.com/business/page1', 'Business Link 1', true, NOW() - INTERVAL '15 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz002', 'https://example.com/business/page2', 'Business Link 2', true, NOW() - INTERVAL '14 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz003', 'https://example.com/business/page3', 'Business Link 3', true, NOW() - INTERVAL '13 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz004', 'https://example.com/business/page4', 'Business Link 4', true, NOW() - INTERVAL '12 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz005', 'https://example.com/business/page5', 'Business Link 5', true, NOW() - INTERVAL '11 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz006', 'https://example.com/business/page6', 'Business Link 6', true, NOW() - INTERVAL '10 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz007', 'https://example.com/business/page7', 'Business Link 7', true, NOW() - INTERVAL '9 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz008', 'https://example.com/business/page8', 'Business Link 8', true, NOW() - INTERVAL '8 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz009', 'https://example.com/business/page9', 'Business Link 9', true, NOW() - INTERVAL '7 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz010', 'https://example.com/business/page10', 'Business Link 10', true, NOW() - INTERVAL '6 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz011', 'https://example.com/business/page11', 'Business Link 11', true, NOW() - INTERVAL '5 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz012', 'https://example.com/business/page12', 'Business Link 12', true, NOW() - INTERVAL '4 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz013', 'https://example.com/business/page13', 'Business Link 13', true, NOW() - INTERVAL '3 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz014', 'https://example.com/business/page14', 'Business Link 14', true, NOW() - INTERVAL '2 days', NOW()),
    (gen_random_uuid(), 'f3333333-3333-3333-3333-333333333333', 'biz015', 'https://example.com/business/page15', 'Business Link 15', true, NOW() - INTERVAL '1 day', NOW());

-- Enterprise user: 20 links (can have unlimited, but starting with 20)
INSERT INTO links (id, user_id, short_code, original_url, title, is_active, created_at, updated_at)
VALUES
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent001', 'https://example.com/enterprise/page1', 'Enterprise Link 1', true, NOW() - INTERVAL '20 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent002', 'https://example.com/enterprise/page2', 'Enterprise Link 2', true, NOW() - INTERVAL '19 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent003', 'https://example.com/enterprise/page3', 'Enterprise Link 3', true, NOW() - INTERVAL '18 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent004', 'https://example.com/enterprise/page4', 'Enterprise Link 4', true, NOW() - INTERVAL '17 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent005', 'https://example.com/enterprise/page5', 'Enterprise Link 5', true, NOW() - INTERVAL '16 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent006', 'https://example.com/enterprise/page6', 'Enterprise Link 6', true, NOW() - INTERVAL '15 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent007', 'https://example.com/enterprise/page7', 'Enterprise Link 7', true, NOW() - INTERVAL '14 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent008', 'https://example.com/enterprise/page8', 'Enterprise Link 8', true, NOW() - INTERVAL '13 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent009', 'https://example.com/enterprise/page9', 'Enterprise Link 9', true, NOW() - INTERVAL '12 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent010', 'https://example.com/enterprise/page10', 'Enterprise Link 10', true, NOW() - INTERVAL '11 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent011', 'https://example.com/enterprise/page11', 'Enterprise Link 11', true, NOW() - INTERVAL '10 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent012', 'https://example.com/enterprise/page12', 'Enterprise Link 12', true, NOW() - INTERVAL '9 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent013', 'https://example.com/enterprise/page13', 'Enterprise Link 13', true, NOW() - INTERVAL '8 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent014', 'https://example.com/enterprise/page14', 'Enterprise Link 14', true, NOW() - INTERVAL '7 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent015', 'https://example.com/enterprise/page15', 'Enterprise Link 15', true, NOW() - INTERVAL '6 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent016', 'https://example.com/enterprise/page16', 'Enterprise Link 16', true, NOW() - INTERVAL '5 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent017', 'https://example.com/enterprise/page17', 'Enterprise Link 17', true, NOW() - INTERVAL '4 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent018', 'https://example.com/enterprise/page18', 'Enterprise Link 18', true, NOW() - INTERVAL '3 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent019', 'https://example.com/enterprise/page19', 'Enterprise Link 19', true, NOW() - INTERVAL '2 days', NOW()),
    (gen_random_uuid(), 'f4444444-4444-4444-4444-444444444444', 'ent020', 'https://example.com/enterprise/page20', 'Enterprise Link 20', true, NOW() - INTERVAL '1 day', NOW());

-- Additional links for FREE tier user (45 more to reach 50 total)
INSERT INTO links (id, user_id, short_code, original_url, title, description, is_active, created_at, updated_at)
VALUES
    -- Social Media Links
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freesoc1', 'https://facebook.com/mypage', 'Facebook Page', 'My business Facebook page', true, NOW() - INTERVAL '20 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freesoc2', 'https://twitter.com/myhandle', 'Twitter Profile', 'Follow me on Twitter', true, NOW() - INTERVAL '19 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freesoc3', 'https://linkedin.com/in/me', 'LinkedIn Profile', 'Professional network', true, NOW() - INTERVAL '18 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freesoc4', 'https://instagram.com/myinsta', 'Instagram', 'My Instagram photos', true, NOW() - INTERVAL '17 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freesoc5', 'https://youtube.com/c/mychannel', 'YouTube Channel', 'Subscribe to my channel', true, NOW() - INTERVAL '16 days', NOW()),
    
    -- Product Links
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeprd1', 'https://shop.example.com/product1', 'Product 1', 'Amazing product for sale', true, NOW() - INTERVAL '15 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeprd2', 'https://shop.example.com/product2', 'Product 2', 'Another great product', true, NOW() - INTERVAL '14 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeprd3', 'https://shop.example.com/product3', 'Product 3', 'Limited time offer', true, NOW() - INTERVAL '13 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeprd4', 'https://shop.example.com/product4', 'Product 4', 'Best seller item', true, NOW() - INTERVAL '12 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeprd5', 'https://shop.example.com/product5', 'Product 5', 'New arrival', true, NOW() - INTERVAL '11 days', NOW()),
    
    -- Blog Posts
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeblog1', 'https://blog.example.com/post1', 'Blog Post 1', 'How to get started', true, NOW() - INTERVAL '10 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeblog2', 'https://blog.example.com/post2', 'Blog Post 2', 'Advanced techniques', true, NOW() - INTERVAL '9 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeblog3', 'https://blog.example.com/post3', 'Blog Post 3', 'Tips and tricks', true, NOW() - INTERVAL '8 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeblog4', 'https://blog.example.com/post4', 'Blog Post 4', 'Industry insights', true, NOW() - INTERVAL '7 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeblog5', 'https://blog.example.com/post5', 'Blog Post 5', 'Latest trends', true, NOW() - INTERVAL '6 days', NOW()),
    
    -- Documentation
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freedoc1', 'https://docs.example.com/guide1', 'API Documentation', 'Complete API guide', true, NOW() - INTERVAL '25 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freedoc2', 'https://docs.example.com/guide2', 'Setup Guide', 'Installation instructions', true, NOW() - INTERVAL '24 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freedoc3', 'https://docs.example.com/guide3', 'Troubleshooting', 'Common issues and fixes', true, NOW() - INTERVAL '23 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freedoc4', 'https://docs.example.com/guide4', 'Best Practices', 'Recommended approaches', true, NOW() - INTERVAL '22 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freedoc5', 'https://docs.example.com/guide5', 'FAQ', 'Frequently asked questions', true, NOW() - INTERVAL '21 days', NOW()),
    
    -- Event Links
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeevt1', 'https://events.example.com/event1', 'Webinar 1', 'Free online webinar', true, NOW() - INTERVAL '30 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeevt2', 'https://events.example.com/event2', 'Workshop', 'Hands-on workshop', true, NOW() - INTERVAL '29 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeevt3', 'https://events.example.com/event3', 'Conference', 'Industry conference', true, NOW() - INTERVAL '28 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeevt4', 'https://events.example.com/event4', 'Meetup', 'Local meetup event', true, NOW() - INTERVAL '27 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeevt5', 'https://events.example.com/event5', 'Demo Day', 'Product demonstration', true, NOW() - INTERVAL '26 days', NOW()),
    
    -- Resource Links
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeres1', 'https://resources.example.com/template1', 'Template 1', 'Free download template', true, NOW() - INTERVAL '35 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeres2', 'https://resources.example.com/template2', 'Template 2', 'Business template', true, NOW() - INTERVAL '34 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeres3', 'https://resources.example.com/template3', 'Template 3', 'Design template', true, NOW() - INTERVAL '33 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeres4', 'https://resources.example.com/ebook1', 'Free eBook', 'Complete guide ebook', true, NOW() - INTERVAL '32 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeres5', 'https://resources.example.com/checklist1', 'Checklist', 'Step-by-step checklist', true, NOW() - INTERVAL '31 days', NOW()),
    
    -- Portfolio/Gallery
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeport1', 'https://portfolio.example.com/project1', 'Project 1', 'Web development project', true, NOW() - INTERVAL '40 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeport2', 'https://portfolio.example.com/project2', 'Project 2', 'Mobile app project', true, NOW() - INTERVAL '39 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeport3', 'https://portfolio.example.com/project3', 'Project 3', 'Design showcase', true, NOW() - INTERVAL '38 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeport4', 'https://portfolio.example.com/project4', 'Project 4', 'Case study', true, NOW() - INTERVAL '37 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freeport5', 'https://portfolio.example.com/project5', 'Project 5', 'Client work', true, NOW() - INTERVAL '36 days', NOW()),
    
    -- Contact/About
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freecon1', 'https://contact.example.com/about', 'About Us', 'Company information', true, NOW() - INTERVAL '45 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freecon2', 'https://contact.example.com/team', 'Our Team', 'Meet the team', true, NOW() - INTERVAL '44 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freecon3', 'https://contact.example.com/contact', 'Contact Form', 'Get in touch', true, NOW() - INTERVAL '43 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freecon4', 'https://contact.example.com/support', 'Support', 'Customer support', true, NOW() - INTERVAL '42 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freecon5', 'https://contact.example.com/careers', 'Careers', 'Join our team', true, NOW() - INTERVAL '41 days', NOW()),
    
    -- News/Updates
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freenews1', 'https://news.example.com/announcement1', 'Announcement 1', 'Product launch news', true, NOW() - INTERVAL '50 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freenews2', 'https://news.example.com/announcement2', 'Announcement 2', 'Company milestone', true, NOW() - INTERVAL '49 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freenews3', 'https://news.example.com/announcement3', 'Announcement 3', 'Partnership news', true, NOW() - INTERVAL '48 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freenews4', 'https://news.example.com/announcement4', 'Announcement 4', 'Feature update', true, NOW() - INTERVAL '47 days', NOW()),
    (gen_random_uuid(), 'f1111111-1111-1111-1111-111111111111', 'freenews5', 'https://news.example.com/announcement5', 'Announcement 5', 'Industry award', true, NOW() - INTERVAL '46 days', NOW());

-- Additional links for PRO tier user (90 more to reach 100 total)
-- Marketing Campaign Links
INSERT INTO links (id, user_id, short_code, original_url, title, description, is_active, created_at, updated_at)
SELECT 
    gen_random_uuid(),
    'f2222222-2222-2222-2222-222222222222',
    'pro' || LPAD(generate_series::text, 3, '0'),
    'https://marketing.example.com/campaign' || generate_series,
    'Campaign ' || generate_series,
    'Marketing campaign link ' || generate_series,
    true,
    NOW() - (generate_series || ' days')::interval,
    NOW()
FROM generate_series(11, 100);

-- Additional links for BUSINESS tier user (185 more to reach 200 total)  
-- E-commerce and Business Links
INSERT INTO links (id, user_id, short_code, original_url, title, description, is_active, created_at, updated_at)
SELECT 
    gen_random_uuid(),
    'f3333333-3333-3333-3333-333333333333',
    'biz' || LPAD(generate_series::text, 3, '0'),
    'https://business.example.com/page' || generate_series,
    'Business Page ' || generate_series,
    'Business content link ' || generate_series,
    true,
    NOW() - (generate_series || ' days')::interval,
    NOW()
FROM generate_series(16, 200);

-- Additional links for ENTERPRISE tier user (480 more to reach 500 total)
-- Enterprise Scale Links
INSERT INTO links (id, user_id, short_code, original_url, title, description, is_active, created_at, updated_at)
SELECT 
    gen_random_uuid(),
    'f4444444-4444-4444-4444-444444444444',
    'ent' || LPAD(generate_series::text, 3, '0'),
    'https://enterprise.example.com/resource' || generate_series,
    'Enterprise Resource ' || generate_series,
    'Enterprise content link ' || generate_series,
    true,
    NOW() - (generate_series || ' days')::interval,
    NOW()
FROM generate_series(21, 500);
