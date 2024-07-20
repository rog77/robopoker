/// we can evaluate a vector of cards lazily by chaining find_* hand rank methods,
/// or we can use ~500MB of memory to store a table of all uniquely evaluated hands.
/// this is a strong tradeoff between space and time complexity.
/// i'll maybe precalculate results and implement LookupEvaluator later

pub trait Evaluator {
    fn strength(cards: &[&Card]) -> Strength;
}

pub struct LazyEvaluator {
    hand_set: u32,         // which ranks are in the hand
    suit_set: [u32; 4],    // which ranks are in suits are in the hand
    rank_counts: [u8; 13], // how many i ranks are in the hand. neglect suit
    suit_counts: [u8; 4],  // how many i suits are in the hand. neglect rank
}

impl Evaluator for LazyEvaluator {
    fn strength(cards: &[&Card]) -> Strength {
        let this = Self {
            hand_set: Self::u32_hand(&cards),
            suit_set: Self::u32_suit(&cards),
            rank_counts: Self::rank_counts(&cards),
            suit_counts: Self::suit_counts(&cards),
        };
        let best_hand = this.find_best_hand();
        let kickers = this.find_kickers(best_hand);
        Strength::new(best_hand, kickers)
    }
}

impl LazyEvaluator {
    fn find_best_hand(&self) -> Value {
        self.find_flush()
            .or_else(|| self.find_4_oak())
            .or_else(|| self.find_3_oak_2_oak())
            .or_else(|| self.find_5_iar())
            .or_else(|| self.find_3_oak())
            .or_else(|| self.find_2_oak_2_oak())
            .or_else(|| self.find_2_oak())
            .or_else(|| self.find_1_oak())
            .unwrap()
    }
    fn find_kickers(&self, strength: Value) -> Kickers {
        let n = match strength {
            Value::HighCard(_) => 4,
            Value::OnePair(_) => 3,
            Value::ThreeOAK(_) => 2,
            Value::FourOAK(_) => 1,
            Value::TwoPair(_, _) => 1,
            _ => return Kickers(Vec::new()),
        };
        Kickers(
            self.rank_counts
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, x)| **x > 0)
                .filter(|(r, _)| *r != strength.primary() as usize)
                .filter(|(r, _)| *r != strength.secondary() as usize)
                .map(|(i, _)| Rank::from(i as u8))
                .take(n)
                .collect::<Vec<Rank>>(),
        )
    }

    fn find_flush(&self) -> Option<Value> {
        self.find_suit_of_flush().and_then(|suit| {
            self.find_rank_of_straight_flush(suit)
                .map(Value::StraightFlush)
                .or_else(|| Some(Value::Flush(Rank::from(self.suit_set[suit as usize]))))
        })
    }
    fn find_5_iar(&self) -> Option<Value> {
        self.find_rank_of_straight(self.hand_set)
            .map(|rank| Value::Straight(rank))
    }
    fn find_3_oak_2_oak(&self) -> Option<Value> {
        self.find_rank_of_n_oak(3).and_then(|triple| {
            self.find_rank_of_n_oak_below(2, triple as usize)
                .map(|couple| Value::FullHouse(triple, couple))
        })
    }
    fn find_2_oak_2_oak(&self) -> Option<Value> {
        self.find_rank_of_n_oak(2).and_then(|high| {
            self.find_rank_of_n_oak_below(2, high as usize)
                .map(|next| Value::TwoPair(high, next))
                .or_else(|| Some(Value::OnePair(high)))
        })
    }
    fn find_4_oak(&self) -> Option<Value> {
        self.find_rank_of_n_oak(4).map(|rank| Value::FourOAK(rank))
    }
    fn find_3_oak(&self) -> Option<Value> {
        self.find_rank_of_n_oak(3).map(|rank| Value::ThreeOAK(rank))
    }
    fn find_2_oak(&self) -> Option<Value> {
        // lowkey unreachable because TwoPair short circuits
        self.find_rank_of_n_oak(2).map(|rank| Value::OnePair(rank))
    }
    fn find_1_oak(&self) -> Option<Value> {
        self.find_rank_of_n_oak(1).map(|rank| Value::HighCard(rank))
    }

    fn find_suit_of_flush(&self) -> Option<Suit> {
        self.suit_counts
            .iter()
            .position(|&n| n >= 5)
            .map(|i| Suit::from(i as u8))
    }
    fn find_rank_of_straight_flush(&self, suit: Suit) -> Option<Rank> {
        let u32_flush = self.suit_set[suit as usize];
        self.find_rank_of_straight(u32_flush)
    }
    fn find_rank_of_straight(&self, u32_cards: u32) -> Option<Rank> {
        let mut mask = u32_cards;
        mask &= mask << 1;
        mask &= mask << 1;
        mask &= mask << 1;
        mask &= mask << 1;
        if mask.count_ones() > 0 {
            return Some(Rank::from(mask));
        } else if (u32_cards & Self::wheel()) == Self::wheel() {
            return Some(Rank::Five);
        } else {
            return None;
        }
    }
    fn find_rank_of_n_oak(&self, n: u8) -> Option<Rank> {
        self.find_rank_of_n_oak_below(n, 13)
    }
    fn find_rank_of_n_oak_below(&self, n: u8, high: usize) -> Option<Rank> {
        self.rank_counts
            .iter()
            .take(high)
            .rev()
            .position(|&r| r >= n)
            .map(|i| high - i - 1)
            .map(|r| Rank::from(r as u8))
    }

    fn rank_counts(cards: &[&Card]) -> [u8; 13] {
        let mut rank_counts = [0; 13];
        cards
            .iter()
            .map(|c| c.rank())
            .map(|r| r as usize)
            .for_each(|r| rank_counts[r] += 1);
        rank_counts
    }
    fn suit_counts(cards: &[&Card]) -> [u8; 4] {
        let mut suit_counts = [0; 4];
        cards
            .iter()
            .map(|c| c.suit())
            .map(|s| s as usize)
            .for_each(|s| suit_counts[s] += 1);
        suit_counts
    }
    fn u32_hand(cards: &[&Card]) -> u32 {
        let mut u32_hand = 0;
        cards
            .iter()
            .map(|c| c.rank())
            .map(|r| u32::from(r))
            .for_each(|r| u32_hand |= r);
        u32_hand
    }
    fn u32_suit(cards: &[&Card]) -> [u32; 4] {
        let mut u32_suit = [0; 4];
        cards
            .iter()
            .map(|c| (c.suit(), c.rank()))
            .map(|(s, r)| (s as usize, u32::from(r)))
            .for_each(|(s, r)| u32_suit[s] |= r);
        u32_suit
    }

    const fn wheel() -> u32 {
        0b00000000000000000001000000001111
    }
}

use super::strength::{Kickers, Strength, Value};
use crate::cards::card::Card;
use crate::cards::rank::Rank;
use crate::cards::suit::Suit;
