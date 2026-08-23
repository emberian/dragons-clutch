//! Exact wrapper-recipe commitment and bounded Merkle-membership contract.
//!
//! Product owns only the authenticated recipe-set identity carried by an
//! attachment. Structured owns recipe bodies and membership semantics. The
//! live adapter supplies SHA-256; this pure contract performs the ordered,
//! fixed-depth verification without Solana account or syscall types.

use clutch_structured_claim::{ClaimVector, MAX_OUTCOMES};

use crate::{put, Error, Result};

/// Maximum number of recipes in the first executable commitment profile.
pub const MAX_WRAPPER_RECIPES_V1: u16 = 16;
/// Fixed array capacity corresponding to [`MAX_WRAPPER_RECIPES_V1`].
pub const MAX_WRAPPER_RECIPE_SLOTS_V1: usize = 16;
/// Exact fixed binary-Merkle depth for sixteen leaves.
pub const WRAPPER_RECIPE_MERKLE_DEPTH_V1: usize = 4;
/// Canonical wire byte for the fixed Merkle depth.
pub const WRAPPER_RECIPE_MERKLE_DEPTH_BYTE_V1: u8 = 4;
/// Recipe identity domain.
pub const WRAPPER_RECIPE_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/wrapper-recipe/v1\0";
/// Ordered Merkle-node domain.
pub const WRAPPER_RECIPE_NODE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/wrapper-recipe-node/v1\0";
/// Recipe-set commitment identity domain.
pub const WRAPPER_RECIPE_SET_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/wrapper-recipe-set/v1\0";
/// Exact recipe preimage width, excluding its hash domain.
pub const WRAPPER_RECIPE_PREIMAGE_BYTES_V1: usize = 40 + (MAX_OUTCOMES * 8);
/// Exact set-commitment preimage width, excluding its hash domain.
pub const WRAPPER_RECIPE_SET_PREIMAGE_BYTES_V1: usize = 48;
/// Exact create-payload membership witness width.
pub const WRAPPER_RECIPE_MEMBERSHIP_BYTES_V1: usize =
    4 + (WRAPPER_RECIPE_MERKLE_DEPTH_V1 * 32);

/// Hash boundary supplied by the target adapter.
pub trait WrapperRecipeHashV1 {
    /// SHA-256 the exact ordered slices without hidden framing.
    fn hashv(&self, slices: &[&[u8]]) -> [u8; 32];
}

/// Exact native-claim wrapper recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapperRecipeV1 {
    /// Canonical native-claim identity reconstructed from Product/Market terms.
    pub native_claim_id: [u8; 32],
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Primitive GCD-one coefficient vector.
    pub primitive: [u64; MAX_OUTCOMES],
}

impl WrapperRecipeV1 {
    /// Encode the canonical recipe identity preimage.
    pub fn encode_preimage(self) -> Result<[u8; WRAPPER_RECIPE_PREIMAGE_BYTES_V1]> {
        if self.native_claim_id == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        ClaimVector {
            outcome_count: self.outcome_count,
            coefficients: self.primitive,
        }
        .validate()
        .map_err(|_| Error::InvalidClaim)?;
        let mut output = [0_u8; WRAPPER_RECIPE_PREIMAGE_BYTES_V1];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.native_claim_id)?;
        put(&mut output, &mut cursor, &[self.outcome_count])?;
        put(&mut output, &mut cursor, &[0; 7])?;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            put(
                &mut output,
                &mut cursor,
                &self.primitive[index].to_le_bytes(),
            )?;
            index += 1;
        }
        if cursor != output.len() {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    /// Derive the exact recipe identity through the supplied SHA-256 boundary.
    pub fn id<H: WrapperRecipeHashV1>(self, hasher: &H) -> Result<[u8; 32]> {
        let body = self.encode_preimage()?;
        let id = hasher.hashv(&[WRAPPER_RECIPE_ID_DOMAIN_V1, &body]);
        if id == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        Ok(id)
    }
}

/// Fixed-depth membership witness carried by Structured create.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapperRecipeMembershipV1 {
    /// Number of live ordered leaves in the committed set.
    pub leaf_count: u16,
    /// Zero-based index of this recipe.
    pub leaf_index: u16,
    /// One sibling per ordered binary-tree level.
    pub siblings: [[u8; 32]; WRAPPER_RECIPE_MERKLE_DEPTH_V1],
}

impl WrapperRecipeMembershipV1 {
    /// Encode the exact wire witness.
    pub fn encode(self) -> Result<[u8; WRAPPER_RECIPE_MEMBERSHIP_BYTES_V1]> {
        self.validate_shape()?;
        let mut output = [0_u8; WRAPPER_RECIPE_MEMBERSHIP_BYTES_V1];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.leaf_count.to_le_bytes())?;
        put(&mut output, &mut cursor, &self.leaf_index.to_le_bytes())?;
        for sibling in self.siblings {
            put(&mut output, &mut cursor, &sibling)?;
        }
        if cursor != output.len() {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    /// Decode one exact witness without accepting trailing bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != WRAPPER_RECIPE_MEMBERSHIP_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        let leaf_count = u16::from_le_bytes([input[0], input[1]]);
        let leaf_index = u16::from_le_bytes([input[2], input[3]]);
        let mut siblings = [[0_u8; 32]; WRAPPER_RECIPE_MERKLE_DEPTH_V1];
        let mut level = 0_usize;
        while level < WRAPPER_RECIPE_MERKLE_DEPTH_V1 {
            let start = 4 + (level * 32);
            siblings[level].copy_from_slice(&input[start..start + 32]);
            level += 1;
        }
        let value = Self {
            leaf_count,
            leaf_index,
            siblings,
        };
        value.validate_shape()?;
        Ok(value)
    }

    fn validate_shape(self) -> Result<()> {
        if self.leaf_count == 0
            || self.leaf_count > MAX_WRAPPER_RECIPES_V1
            || self.leaf_index >= self.leaf_count
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
}

/// Authenticate one recipe and reconstruct the exact Product-owned set ID.
pub fn authenticate_wrapper_recipe_membership_v1<H: WrapperRecipeHashV1>(
    recipe: WrapperRecipeV1,
    expected_recipe_id: [u8; 32],
    membership: WrapperRecipeMembershipV1,
    expected_recipe_set_id: [u8; 32],
    hasher: &H,
) -> Result<()> {
    membership.validate_shape()?;
    if expected_recipe_id == [0; 32]
        || expected_recipe_set_id == [0; 32]
        || recipe.id(hasher)? != expected_recipe_id
    {
        return Err(Error::InvalidIdentity);
    }
    let mut node = expected_recipe_id;
    let original_index = usize::from(membership.leaf_index);
    let mut index = original_index;
    let leaf_count = usize::from(membership.leaf_count);
    let mut empty_subtree = [0_u8; 32];
    let mut level = 0_usize;
    while level < WRAPPER_RECIPE_MERKLE_DEPTH_V1 {
        let sibling = membership.siblings[level];
        let subtree_width = 1_usize << level;
        let subtree_start = (original_index >> level) << level;
        let sibling_start = subtree_start ^ subtree_width;
        if sibling_start >= leaf_count && sibling != empty_subtree {
            return Err(Error::InvalidIdentity);
        }
        node = if index & 1 == 0 {
            hasher.hashv(&[WRAPPER_RECIPE_NODE_DOMAIN_V1, &node, &sibling])
        } else {
            hasher.hashv(&[WRAPPER_RECIPE_NODE_DOMAIN_V1, &sibling, &node])
        };
        if node == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        empty_subtree = hasher.hashv(&[
            WRAPPER_RECIPE_NODE_DOMAIN_V1,
            &empty_subtree,
            &empty_subtree,
        ]);
        index >>= 1;
        level += 1;
    }
    let set_body = encode_set_preimage(membership.leaf_count, node)?;
    let set_id = hasher.hashv(&[WRAPPER_RECIPE_SET_ID_DOMAIN_V1, &set_body]);
    if set_id != expected_recipe_set_id {
        return Err(Error::InvalidIdentity);
    }
    Ok(())
}

/// Build one canonical set identity and membership proof from an exact ordered
/// fixed-capacity recipe list. Live leaves must be nonzero and distinct; every
/// inactive tail slot must be zero.
pub fn build_wrapper_recipe_membership_v1<H: WrapperRecipeHashV1>(
    recipe_ids: [[u8; 32]; MAX_WRAPPER_RECIPE_SLOTS_V1],
    leaf_count: u16,
    leaf_index: u16,
    hasher: &H,
) -> Result<([u8; 32], WrapperRecipeMembershipV1)> {
    if leaf_count == 0
        || leaf_count > MAX_WRAPPER_RECIPES_V1
        || leaf_index >= leaf_count
    {
        return Err(Error::InvalidIdentity);
    }
    let active = usize::from(leaf_count);
    let mut left = 0_usize;
    while left < recipe_ids.len() {
        if (left < active) != (recipe_ids[left] != [0; 32]) {
            return Err(Error::InvalidIdentity);
        }
        if left < active {
            let mut right = left + 1;
            while right < active {
                if recipe_ids[left] == recipe_ids[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
        }
        left += 1;
    }
    let mut nodes = recipe_ids;
    let mut membership = WrapperRecipeMembershipV1 {
        leaf_count,
        leaf_index,
        siblings: [[0; 32]; WRAPPER_RECIPE_MERKLE_DEPTH_V1],
    };
    let mut index = usize::from(leaf_index);
    let mut width = MAX_WRAPPER_RECIPE_SLOTS_V1;
    let mut level = 0_usize;
    while level < WRAPPER_RECIPE_MERKLE_DEPTH_V1 {
        membership.siblings[level] = nodes[index ^ 1];
        let mut parent = 0_usize;
        while parent < width / 2 {
            nodes[parent] = hasher.hashv(&[
                WRAPPER_RECIPE_NODE_DOMAIN_V1,
                &nodes[parent * 2],
                &nodes[(parent * 2) + 1],
            ]);
            if nodes[parent] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            parent += 1;
        }
        index >>= 1;
        width /= 2;
        level += 1;
    }
    let set_body = encode_set_preimage(leaf_count, nodes[0])?;
    let set_id = hasher.hashv(&[WRAPPER_RECIPE_SET_ID_DOMAIN_V1, &set_body]);
    if set_id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok((set_id, membership))
}

fn encode_set_preimage(
    leaf_count: u16,
    root: [u8; 32],
) -> Result<[u8; WRAPPER_RECIPE_SET_PREIMAGE_BYTES_V1]> {
    if leaf_count == 0 || leaf_count > MAX_WRAPPER_RECIPES_V1 || root == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    let mut output = [0_u8; WRAPPER_RECIPE_SET_PREIMAGE_BYTES_V1];
    let mut cursor = 0_usize;
    put(&mut output, &mut cursor, b"DCRSETV1")?;
    put(&mut output, &mut cursor, &leaf_count.to_le_bytes())?;
    put(
        &mut output,
        &mut cursor,
        &[WRAPPER_RECIPE_MERKLE_DEPTH_BYTE_V1],
    )?;
    put(&mut output, &mut cursor, &[0; 5])?;
    put(&mut output, &mut cursor, &root)?;
    if cursor != output.len() {
        return Err(Error::InvalidLength);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DeterministicHash;

    impl WrapperRecipeHashV1 for DeterministicHash {
        fn hashv(&self, slices: &[&[u8]]) -> [u8; 32] {
            let mut output = [0_u8; 32];
            let mut index = 0_usize;
            for slice in slices {
                for byte in *slice {
                    output[index & 31] = output[index & 31]
                        .wrapping_mul(31)
                        .wrapping_add(*byte);
                    index += 1;
                }
            }
            output[0] |= 1;
            output
        }
    }

    fn recipe() -> WrapperRecipeV1 {
        let mut primitive = [0_u64; MAX_OUTCOMES];
        primitive[0] = 1;
        primitive[1] = 2;
        WrapperRecipeV1 {
            native_claim_id: [7; 32],
            outcome_count: 2,
            primitive,
        }
    }

    #[test]
    fn proof_refuses_wrong_index_count_recipe_and_set() {
        let hash = DeterministicHash;
        let recipe = recipe();
        let recipe_id = recipe.id(&hash).unwrap();
        let mut recipes = [[0_u8; 32]; MAX_WRAPPER_RECIPE_SLOTS_V1];
        recipes[0] = recipe_id;
        let (set_id, membership) =
            build_wrapper_recipe_membership_v1(recipes, 1, 0, &hash).unwrap();
        assert_eq!(
            authenticate_wrapper_recipe_membership_v1(
                recipe,
                recipe_id,
                membership,
                set_id,
                &hash,
            ),
            Ok(())
        );
        let mut wrong = membership;
        wrong.leaf_count = 0;
        assert_eq!(
            authenticate_wrapper_recipe_membership_v1(recipe, recipe_id, wrong, set_id, &hash),
            Err(Error::InvalidIdentity)
        );
        let mut hidden_tail = membership;
        hidden_tail.siblings[0] = [3; 32];
        assert_eq!(
            authenticate_wrapper_recipe_membership_v1(
                recipe,
                recipe_id,
                hidden_tail,
                set_id,
                &hash,
            ),
            Err(Error::InvalidIdentity)
        );
        recipes[1] = recipe_id;
        assert_eq!(
            build_wrapper_recipe_membership_v1(recipes, 2, 0, &hash),
            Err(Error::InvalidIdentity)
        );
        assert_eq!(
            authenticate_wrapper_recipe_membership_v1(
                recipe,
                [8; 32],
                membership,
                set_id,
                &hash,
            ),
            Err(Error::InvalidIdentity)
        );
        assert_eq!(
            authenticate_wrapper_recipe_membership_v1(
                recipe,
                recipe_id,
                membership,
                [9; 32],
                &hash,
            ),
            Err(Error::InvalidIdentity)
        );
    }
}
