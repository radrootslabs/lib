import { z } from "zod";

export const radroots_nostr_event_ref_schema = z.object({
    id: z.string(),
    author: z.string(),
    kind: z.number(),
    d_tag: z.string().optional(),
    relays: z.array(z.string()).optional()
});

export const radroots_listing_image_schema = z.object({
    url: z.string(),
    size: z.object({
            w: z.number(),
            h: z.number()
        }).optional()
});

export const radroots_listing_location_schema = z.object({
    primary: z.string(),
    city: z.string().optional(),
    region: z.string().optional(),
    country: z.string().optional(),
    lat: z.number().optional(),
    lng: z.number().optional(),
    geohash: z.string().optional()
});

export const radroots_listing_discount_schema = z.union([
    z.object({
        quantity: z.object({
                    ref_quantity: z.string(),
                    threshold: z.string(),
                    value: z.string(),
                    currency: z.string()
                })
    }),
    z.object({
        mass: z.object({
                    unit: z.string(),
                    threshold: z.string(),
                    threshold_unit: z.string(),
                    value: z.string(),
                    currency: z.string()
                })
    }),
    z.object({
        subtotal: z.object({
                    threshold: z.string(),
                    currency: z.string(),
                    value: z.string(),
                    measure: z.string()
                })
    }),
    z.object({
        total: z.object({
                    total_min: z.string(),
                    value: z.string(),
                    measure: z.string()
                })
    })
]);

export const radroots_listing_price_schema = z.object({
    amt: z.string(),
    currency: z.string(),
    qty_amt: z.string(),
    qty_unit: z.string(),
    qty_key: z.string()
});

export const radroots_listing_quantity_schema = z.object({
    amt: z.string(),
    unit: z.string(),
    label: z.string().optional()
});

export const radroots_listing_product_schema = z.object({
    key: z.string(),
    title: z.string(),
    category: z.string(),
    summary: z.string().optional(),
    process: z.string().optional(),
    lot: z.string().optional(),
    location: z.string().optional(),
    profile: z.string().optional(),
    year: z.string().optional()
});

export const radroots_listing_schema = z.object({
    d_tag: z.string(),
    product: radroots_listing_product_schema,
    quantities: z.array(radroots_listing_quantity_schema),
    prices: z.array(radroots_listing_price_schema),
    discounts: z.array(radroots_listing_discount_schema).optional(),
    location: radroots_listing_location_schema.optional(),
    images: z.array(radroots_listing_image_schema).optional()
});

export const radroots_profile_schema = z.object({
    name: z.string(),
    display_name: z.string().optional(),
    nip05: z.string().optional(),
    about: z.string().optional(),
    website: z.string().optional(),
    picture: z.string().optional(),
    banner: z.string().optional(),
    lud06: z.string().optional(),
    lud16: z.string().optional(),
    bot: z.string().optional()
});

export const radroots_comment_schema = z.object({
    root: radroots_nostr_event_ref_schema,
    parent: radroots_nostr_event_ref_schema,
    content: z.string()
});

export const radroots_reaction_schema = z.object({
    root: radroots_nostr_event_ref_schema,
    content: z.string()
});

export const radroots_follow_profile_schema = z.object({
    published_at: z.number(),
    public_key: z.string(),
    relay_url: z.string().optional(),
    contact_name: z.string().optional()
});

export const radroots_follow_schema = z.object({
    list: z.array(radroots_follow_profile_schema)
});
