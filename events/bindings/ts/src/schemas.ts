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

export const radroots_nostr_event_ptr_schema = z.object({
    id: z.string(),
    relays: z.string().optional()
});

export const radroots_message_recipient_schema = z.object({
    public_key: z.string(),
    relay_url: z.string().optional()
});

export const radroots_message_schema = z.object({
    recipients: z.array(radroots_message_recipient_schema),
    content: z.string(),
    reply_to: radroots_nostr_event_ptr_schema.optional(),
    subject: z.string().optional()
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

export const radroots_list_entry_schema = z.object({
    tag: z.string(),
    values: z.array(z.string())
});

export const radroots_list_schema = z.object({
    content: z.string(),
    entries: z.array(radroots_list_entry_schema)
});

export const radroots_list_set_schema = z.object({
    d_tag: z.string(),
    content: z.string(),
    entries: z.array(radroots_list_entry_schema),
    title: z.string().optional(),
    description: z.string().optional(),
    image: z.string().optional()
});

export const radroots_farm_location_schema = z.object({
    primary: z.string(),
    city: z.string().optional(),
    region: z.string().optional(),
    country: z.string().optional(),
    lat: z.number().optional(),
    lng: z.number().optional(),
    geohash: z.string().optional()
});

export const radroots_farm_schema = z.object({
    d_tag: z.string(),
    name: z.string(),
    about: z.string().optional(),
    website: z.string().optional(),
    picture: z.string().optional(),
    banner: z.string().optional(),
    location: radroots_farm_location_schema.optional(),
    tags: z.array(z.string()).optional()
});

export const radroots_farm_ref_schema = z.object({
    pubkey: z.string(),
    d_tag: z.string()
});

export const radroots_plot_location_schema = z.object({
    primary: z.string(),
    city: z.string().optional(),
    region: z.string().optional(),
    country: z.string().optional(),
    lat: z.number().optional(),
    lng: z.number().optional(),
    geohash: z.string().optional()
});

export const radroots_plot_schema = z.object({
    d_tag: z.string(),
    farm: radroots_farm_ref_schema,
    name: z.string(),
    about: z.string().optional(),
    location: radroots_plot_location_schema.optional(),
    geometry: z.string().optional(),
    tags: z.array(z.string()).optional()
});

export const radroots_plot_farm_ref_schema = radroots_farm_ref_schema;
