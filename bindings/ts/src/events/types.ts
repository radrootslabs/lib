import { z } from "zod";
import { radroots_comment_schema, radroots_follow_profile_schema, radroots_follow_schema, radroots_listing_discount_schema, radroots_listing_image_schema, radroots_listing_location_schema, radroots_listing_price_schema, radroots_listing_product_schema, radroots_listing_quantity_schema, radroots_listing_schema, radroots_nostr_event_ref_schema, radroots_profile_schema, radroots_reaction_schema } from "./schema.js";

export type RadrootsNostrEventRef = z.infer<typeof radroots_nostr_event_ref_schema>;
export type RadrootsListingImage = z.infer<typeof radroots_listing_image_schema>;
export type RadrootsListingLocation = z.infer<typeof radroots_listing_location_schema>;
export type RadrootsListingDiscount = z.infer<typeof radroots_listing_discount_schema>;
export type RadrootsListingPrice = z.infer<typeof radroots_listing_price_schema>;
export type RadrootsListingQuantity = z.infer<typeof radroots_listing_quantity_schema>;
export type RadrootsListingProduct = z.infer<typeof radroots_listing_product_schema>;
export type RadrootsListing = z.infer<typeof radroots_listing_schema>;
export type RadrootsProfile = z.infer<typeof radroots_profile_schema>;
export type RadrootsComment = z.infer<typeof radroots_comment_schema>;
export type RadrootsReaction = z.infer<typeof radroots_reaction_schema>;
export type RadrootsFollowProfile = z.infer<typeof radroots_follow_profile_schema>;
export type RadrootsFollow = z.infer<typeof radroots_follow_schema>;