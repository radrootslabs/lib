import { z } from "zod";

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
        kind: z.literal("quantity"),
        amount: z.object({
                    ref_quantity: z.string(),
                    threshold: z.any(),
                    value: z.any()
                })
    }),
    z.object({
        kind: z.literal("mass"),
        amount: z.object({
                    threshold: z.any(),
                    value: z.any()
                })
    }),
    z.object({
        kind: z.literal("subtotal"),
        amount: z.object({
                    threshold: z.any(),
                    value: z.any()
                })
    }),
    z.object({
        kind: z.literal("total"),
        amount: z.object({
                    total_min: z.any(),
                    value: z.any()
                })
    })
]);

export const radroots_listing_price_schema = z.object({
    amount: z.any(),
    quantity: z.any()
});

export const radroots_listing_quantity_schema = z.object({
    value: z.any(),
    label: z.string().optional(),
    count: z.number().optional()
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
    root: z.any(),
    parent: z.any(),
    content: z.string()
});

export const radroots_reaction_schema = z.object({
    root: z.any(),
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
