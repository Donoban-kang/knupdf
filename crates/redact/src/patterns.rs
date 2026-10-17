//! Search & Redact patterns (Acrobat's Find Text ▸ Patterns): phone numbers, email addresses,
//! credit card numbers, US Social Security numbers and dates, and (KnuPdf) Korean personal
//! information: resident/foreigner registration, phone, business registration, bank account,
//! passport and driver's licence numbers, alone or all at once. Each matcher works on a page's
//! text as characters and returns character ranges; callers map them to glyphs and areas.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// KnuPdf: every Korean pattern below plus email addresses and credit cards, in one search.
    KrAll,
    KrResident,
    KrPhone,
    KrBusiness,
    KrAccount,
    KrPassport,
    KrDriver,
    Phone,
    Email,
    CreditCard,
    Ssn,
    Date,
}

pub const PATTERNS: [Pattern; 12] = [
    Pattern::KrAll,
    Pattern::KrResident,
    Pattern::KrPhone,
    Pattern::KrBusiness,
    Pattern::KrAccount,
    Pattern::KrPassport,
    Pattern::KrDriver,
    Pattern::Phone,
    Pattern::Email,
    Pattern::CreditCard,
    Pattern::Ssn,
    Pattern::Date,
];

impl Pattern {
    pub fn label(self) -> &'static str {
        match self {
            Pattern::KrAll => "All Korean personal information",
            Pattern::KrResident => "Korean resident or foreigner registration numbers",
            Pattern::KrPhone => "Korean phone numbers",
            Pattern::KrBusiness => "Korean business registration numbers",
            Pattern::KrAccount => "Korean bank account numbers",
            Pattern::KrPassport => "Korean passport numbers",
            Pattern::KrDriver => "Korean driver's licence numbers",
            Pattern::Phone => "Phone Numbers",
            Pattern::Email => "Email Addresses",
            Pattern::CreditCard => "Credit Cards",
            Pattern::Ssn => "Social Security Numbers",
            Pattern::Date => "Dates",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Pattern::KrAll => "kr-all",
            Pattern::KrResident => "kr-resident",
            Pattern::KrPhone => "kr-phone",
            Pattern::KrBusiness => "kr-business",
            Pattern::KrAccount => "kr-account",
            Pattern::KrPassport => "kr-passport",
            Pattern::KrDriver => "kr-driver",
            Pattern::Phone => "phone",
            Pattern::Email => "email",
            Pattern::CreditCard => "credit-card",
            Pattern::Ssn => "ssn",
            Pattern::Date => "date",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        PATTERNS.into_iter().find(|p| p.id() == id)
    }
}

fn digit(c: Option<&char>) -> bool {
    c.is_some_and(char::is_ascii_digit)
}

fn word(c: Option<&char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric())
}

/// Digits and the separators allowed between them, from `start`: (end, digit count).
fn digit_run(s: &[char], start: usize, seps: &[char], max_len: usize) -> (usize, usize) {
    let (mut i, mut n) = (start, 0);
    while i < s.len() && i - start < max_len {
        let c = s[i];
        if c.is_ascii_digit() {
            n += 1;
        } else if !(seps.contains(&c) && (digit(s.get(i + 1)) || s.get(i + 1) == Some(&'(')) || (c == '(' || c == ')') && seps.contains(&c)) {
            break;
        }
        i += 1;
    }
    // Never end on a separator.
    while i > start && !s[i - 1].is_ascii_digit() {
        i -= 1;
    }
    (i, n)
}

/// Whether the separated digit run goes on past `k` in the direction `step` (1 or −1): the next
/// digit after `k` is reached through separators only.
fn run_continues(s: &[char], k: usize, step: isize, seps: &[char]) -> bool {
    let mut j = k;
    loop {
        let Some(c) = j.checked_add_signed(step).and_then(|j| s.get(j)) else { return false };
        if c.is_ascii_digit() {
            return true;
        }
        if !seps.contains(c) {
            return false;
        }
        j = j.wrapping_add_signed(step);
    }
}

fn phone(s: &[char], i: usize) -> Option<usize> {
    const SEPS: [char; 5] = [' ', '-', '.', '(', ')'];
    let start_ok = s[i] == '+' && digit(s.get(i + 1)) || s[i] == '(' && digit(s.get(i + 1)) || s[i].is_ascii_digit();
    if !start_ok || word(i.checked_sub(1).and_then(|p| s.get(p))) || s.get(i.wrapping_sub(1)) == Some(&'+') {
        return None;
    }
    // A phone number is a whole run of digit groups. The tail of a longer one (an IBAN, a
    // company or social security number) is not a phone number, however it is grouped (#1884).
    if run_continues(s, i, -1, &SEPS) {
        return None;
    }
    let plus = s[i] == '+';
    let first = if plus { i + 1 } else { i };
    let (end, n) = digit_run(s, first, &SEPS, 28);
    let run = s.get(first..end)?;
    let seps = run.iter().filter(|c| !c.is_ascii_digit()).count();
    // Always grouped by at least one separator, so plain numbers never match; nor does the
    // head of a run too long to be a phone number.
    if seps == 0 || digit(s.get(end)) || run_continues(s, end - 1, 1, &SEPS) {
        return None;
    }
    let ok = if plus {
        // E.164: a `+` country code and up to 15 digits in all ("+49 2151 123456").
        (8..=15).contains(&n)
    } else {
        // North American: 10 digits, 11 with a country code ("(555) 123-4567").
        let nanp = (10..=11).contains(&n);
        // National with a trunk 0 and 6–14 more digits ("0211 123456", "01 23 45 67 89").
        // The first group needs two digits so decimals ("0.1234567") don't match, and
        // day-first dates ("01.10.2026") are left to the date pattern.
        let mut digits = run.iter().skip_while(|c| !c.is_ascii_digit());
        let trunk_zero = digits.clone().next() == Some(&'0');
        let lead = digits.by_ref().take_while(|c| c.is_ascii_digit()).count();
        let trunk = trunk_zero && (7..=15).contains(&n) && lead >= 2 && date(s, i).is_none();
        nanp || trunk
    };
    ok.then_some(end - i)
}

/// Scripts written without spaces between words (Chinese, Japanese, Korean, Thai, …): text in
/// them often runs straight into an address, so their letters never count as part of one.
fn spaceless(c: char) -> bool {
    matches!(
        u32::from(c),
        0x0E00..=0x0EFF | 0x1000..=0x109F | 0x1100..=0x11FF | 0x1780..=0x17FF | 0x2E80..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF
    )
}

/// A letter or digit of an address: ASCII, or of any script written with spaces, so
/// internationalized addresses (RFC 6531, IDN) such as "ayşe@örnek.com.tr" match.
fn address_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c.is_alphanumeric() && !spaceless(c)
}

fn email(s: &[char], i: usize) -> Option<usize> {
    let local = |c: &char| address_char(*c) || "._%+-".contains(*c);
    if !address_char(s[i]) || i.checked_sub(1).and_then(|p| s.get(p)).is_some_and(local) {
        return None;
    }
    let mut j = i;
    while j < s.len() && local(&s[j]) {
        j += 1;
    }
    if s.get(j) != Some(&'@') {
        return None;
    }
    let d0 = j + 1;
    let mut k = d0;
    while k < s.len() && (address_char(s[k]) || s[k] == '-' || s[k] == '.' && s.get(k + 1).is_some_and(|c| address_char(*c))) {
        k += 1;
    }
    let domain: String = s[d0..k].iter().collect();
    let tld = domain.rsplit('.').next().unwrap_or("");
    (domain.contains('.') && tld.chars().count() >= 2 && tld.chars().all(char::is_alphabetic)).then_some(k - i)
}

fn luhn(digits: &[u32]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(k, d)| {
            if k % 2 == 1 {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            } else {
                *d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn credit_card(s: &[char], i: usize) -> Option<usize> {
    if !s[i].is_ascii_digit() || digit(i.checked_sub(1).and_then(|p| s.get(p))) {
        return None;
    }
    let (end, n) = digit_run(s, i, &[' ', '-'], 23);
    if !(13..=19).contains(&n) || digit(s.get(end)) {
        return None;
    }
    let digits: Vec<u32> = s[i..end].iter().filter_map(|c| c.to_digit(10)).collect();
    luhn(&digits).then_some(end - i)
}

fn ssn(s: &[char], i: usize) -> Option<usize> {
    if digit(i.checked_sub(1).and_then(|p| s.get(p))) {
        return None;
    }
    let g = |from: usize, n: usize| (from..from + n).all(|k| digit(s.get(k)));
    let sep = |k: usize| matches!(s.get(k), Some('-' | ' '));
    if !(g(i, 3) && sep(i + 3) && g(i + 4, 2) && sep(i + 6) && g(i + 7, 4)) || digit(s.get(i + 11)) {
        return None;
    }
    let area: String = s[i..i + 3].iter().collect();
    (area != "000" && area != "666" && !area.starts_with('9')).then_some(11)
}

/// Month names the date pattern knows: English, then the languages whose dates are written
/// "5 Ekim 2024" / "5. Oktober 2024" (day, month name, year).
const MONTHS: [[&str; 12]; 5] = [
    ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"],
    ["ocak", "şubat", "mart", "nisan", "mayıs", "haziran", "temmuz", "ağustos", "eylül", "ekim", "kasım", "aralık"],
    ["januar", "februar", "märz", "april", "mai", "juni", "juli", "august", "september", "oktober", "november", "dezember"],
    ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
    ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
];

/// Abbreviations that may end in a full stop: English and Turkish ("Oca.", "Şub", "Ağu").
const MONTH_ABBREVIATIONS: [[&str; 12]; 2] = [
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"],
    ["oca", "şub", "mar", "nis", "may", "haz", "tem", "ağu", "eyl", "eki", "kas", "ara"],
];

/// One lowercase character for `c`, with Turkish dotted and dotless i the same as i, so
/// "MAYIS", "Mayıs" and "EKİM" match and every character stays one character.
fn fold(c: char) -> char {
    match c {
        'I' | '\u{130}' | '\u{131}' => 'i',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

/// Characters `name` covers at `i`, when it stands there as a whole word (any case).
fn word_at(s: &[char], i: usize, name: &str) -> Option<usize> {
    let mut n = 0;
    for c in name.chars() {
        if s.get(i + n).copied().map(fold) != Some(fold(c)) {
            return None;
        }
        n += 1;
    }
    (!s.get(i + n).is_some_and(|c| c.is_alphabetic())).then_some(n)
}

/// A month name or its abbreviation (optionally with a full stop) at `i`.
fn month(s: &[char], i: usize) -> Option<usize> {
    if word(i.checked_sub(1).and_then(|p| s.get(p))) {
        return None;
    }
    if let Some(n) = MONTHS.iter().flatten().filter_map(|m| word_at(s, i, m)).max() {
        return Some(n);
    }
    let n = MONTH_ABBREVIATIONS.iter().flatten().find_map(|m| word_at(s, i, m))?;
    Some(if s.get(i + n) == Some(&'.') { n + 1 } else { n })
}

fn number(s: &[char], i: usize, min: usize, max: usize) -> Option<usize> {
    let n = s[i.min(s.len())..].iter().take(max + 1).take_while(|c| c.is_ascii_digit()).count();
    (n >= min && n <= max).then_some(n)
}

fn date(s: &[char], i: usize) -> Option<usize> {
    if word(i.checked_sub(1).and_then(|p| s.get(p))) {
        return None;
    }
    let spaces = |k: usize| s[k.min(s.len())..].iter().take_while(|c| **c == ' ').count();
    // Numeric: m/d/yy(yy), m-d-yyyy, d.m.yyyy, yyyy-mm-dd.
    if let Some(a) = number(s, i, 1, 4) {
        let sep = s.get(i + a).copied();
        if matches!(sep, Some('/' | '-' | '.'))
            && let Some(b) = number(s, i + a + 1, 1, 2)
            && s.get(i + a + 1 + b).copied() == sep
            && let Some(c) = number(s, i + a + b + 2, if a == 4 { 1 } else { 2 }, if a == 4 { 2 } else { 4 })
            && (a <= 2 || a == 4)
            && !digit(s.get(i + a + b + 2 + c))
        {
            return Some(a + b + c + 2);
        }
        // "5 January 2024", "5 Ekim 2024", "5. Oktober 2024".
        let day = i + a + usize::from(s.get(i + a) == Some(&'.'));
        if a <= 2
            && let k = day + spaces(day)
            && k > day
            && let Some(m) = month(s, k)
        {
            let k2 = k + m + spaces(k + m);
            if let Some(y) = number(s, k2, 4, 4) {
                return Some(k2 + y - i);
            }
        }
        return None;
    }
    // "January 5, 2024" / "Jan. 5 2024".
    let m = month(s, i)?;
    let k = i + m + spaces(i + m);
    let d = number(s, k, 1, 2)?;
    let mut k2 = k + d;
    if s.get(k2) == Some(&',') {
        k2 += 1;
    }
    k2 += spaces(k2);
    let y = number(s, k2, 4, 4)?;
    Some(k2 + y - i)
}


// ---- Korean personal information (KnuPdf) -----------------------------------------------------

/// Hyphen-like characters Korean documents put between number groups.
fn dash(c: Option<&char>) -> bool {
    matches!(c, Some('-' | '‐' | '‑' | '–' | '—'))
}

/// A number may start at `i`: the character before is not a digit, nor a dash that follows one
/// (the middle of a longer dashed number).
fn number_starts(s: &[char], i: usize) -> bool {
    let prev = i.checked_sub(1).and_then(|p| s.get(p));
    !digit(prev) && !(dash(prev) && digit(i.checked_sub(2).and_then(|p| s.get(p))))
}

/// A number may end at `end`: the next character is not a digit, nor a dash followed by one.
fn number_ends(s: &[char], end: usize) -> bool {
    !digit(s.get(end)) && !(dash(s.get(end)) && digit(s.get(end + 1)))
}

/// Exactly `n` digits at `i` (no more), as their values.
fn group(s: &[char], i: usize, n: usize) -> Option<Vec<u32>> {
    let d: Vec<u32> = (i..i + n).map(|k| s.get(k).and_then(|c| c.to_digit(10))).collect::<Option<_>>()?;
    Some(d)
}

/// 주민등록번호 / 외국인등록번호: YYMMDD-GNNNNNN. G is 1–8 (5–8 for foreigners); the month and
/// day must be possible. The last six may be masked (`*`, `●`, `x`), as on many printed forms,
/// since the date of birth and gender still identify the person. Numbers issued since October
/// 2020 carry no check digit, so none is tested.
fn kr_resident(s: &[char], i: usize) -> Option<usize> {
    if !number_starts(s, i) {
        return None;
    }
    let birth = group(s, i, 6)?;
    let month = birth.get(2)? * 10 + birth.get(3)?;
    let day = birth.get(4)? * 10 + birth.get(5)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let sep = usize::from(dash(s.get(i + 6)) || s.get(i + 6) == Some(&' '));
    let g = i + 6 + sep;
    if !matches!(s.get(g).and_then(|c| c.to_digit(10)), Some(1..=8)) {
        return None;
    }
    let masked = |c: Option<&char>| matches!(c, Some('*' | '●' | '○' | '■' | 'x' | 'X' | '＊'));
    let mut has_mask = false;
    for k in g + 1..g + 7 {
        let c = s.get(k);
        if masked(c) {
            has_mask = true;
        } else if !digit(c) {
            return None;
        }
    }
    let end = g + 7;
    // Without a separator only a full 13-digit number counts.
    if (sep == 0 && has_mask) || !number_ends(s, end) || masked(s.get(end)) {
        return None;
    }
    Some(end - i)
}

/// Korean telephone numbers: mobile (010, 011, 016–019), Seoul 02, regional 031–064, 070, 050x
/// and 15xx–18xx company numbers, with `-`, `.`, ` ` or `)` between groups, an optional `+82`
/// (then without the trunk 0), and optional brackets round the area code. Only a mobile number
/// may be written without separators (01012345678).
fn kr_phone(s: &[char], i: usize) -> Option<usize> {
    let sep = |c: Option<&char>| dash(c) || matches!(c, Some('.' | ' '));
    if !number_starts(s, i) && s.get(i) != Some(&'+') && s.get(i) != Some(&'(') {
        return None;
    }
    if s.get(i).is_some_and(|c| c.is_alphanumeric()) && word(i.checked_sub(1).and_then(|p| s.get(p))) {
        return None;
    }
    let mut k = i;
    let mut intl = false;
    if s.get(k) == Some(&'+') {
        if s.get(k + 1) != Some(&'8') || s.get(k + 2) != Some(&'2') {
            return None;
        }
        intl = true;
        k += 3;
        if sep(s.get(k)) {
            k += 1;
        }
    }
    let bracket = s.get(k) == Some(&'(');
    if bracket {
        k += 1;
    }
    // Company numbers: 1588-1234 (no area code, never international).
    if !intl
        && !bracket
        && let Some(head) = group(s, k, 4)
        && head.first() == Some(&1)
        && matches!(head.get(1), Some(5..=8))
        && dash(s.get(k + 4))
        && group(s, k + 5, 4).is_some()
        && number_ends(s, k + 9)
    {
        return Some(k + 9 - i);
    }
    // The area code without its trunk 0 after +82, with it otherwise.
    let zero = usize::from(!intl);
    if !intl && s.get(k) != Some(&'0') {
        return None;
    }
    let a = k + zero;
    let d = |n: usize| s.get(a + n).and_then(|c| c.to_digit(10));
    let (area_len, mobile) = match (d(0), d(1), d(2)) {
        (Some(1), Some(0 | 1 | 6..=9), _) => (2, true),
        (Some(2), _, _) => (1, false),
        (Some(3), Some(1..=3), _) | (Some(4), Some(1..=4), _) | (Some(5), Some(1..=5), _) | (Some(6), Some(1..=4), _) => (2, false),
        (Some(7), Some(0), _) => (2, false),
        (Some(5), Some(0), Some(_)) => (3, false),
        _ => return None,
    };
    let mut k = a + area_len;
    if bracket {
        if s.get(k) != Some(&')') {
            return None;
        }
        k += 1;
        if s.get(k) == Some(&' ') {
            k += 1;
        }
    } else if sep(s.get(k)) {
        k += 1;
    } else if !(mobile && !intl) {
        return None;
    }
    let plain = !bracket && !sep(s.get(a + area_len));
    let mid = if group(s, k, 4).is_some() && !digit(s.get(k + 4)) || plain && group(s, k, 8).is_some() { 4 } else { 3 };
    group(s, k, mid)?;
    k += mid;
    if sep(s.get(k)) {
        if plain {
            return None;
        }
        k += 1;
    } else if !plain {
        return None;
    }
    group(s, k, 4)?;
    k += 4;
    number_ends(s, k).then_some(k - i)
}

/// 사업자등록번호: NNN-NN-NNNNN with its check digit.
fn kr_business(s: &[char], i: usize) -> Option<usize> {
    if !number_starts(s, i) {
        return None;
    }
    let a = group(s, i, 3)?;
    let b = group(s, i + 4, 2)?;
    let c = group(s, i + 7, 5)?;
    if !(dash(s.get(i + 3)) && dash(s.get(i + 6)) && number_ends(s, i + 12)) {
        return None;
    }
    let d: Vec<u32> = a.into_iter().chain(b).chain(c).collect();
    let w = [1, 3, 7, 1, 3, 7, 1, 3, 5];
    let mut sum: u32 = d.iter().zip(w).map(|(x, w)| x * w).sum();
    sum += d.get(8)? * 5 / 10;
    ((10 - sum % 10) % 10 == *d.get(9)?).then_some(12)
}

/// 운전면허번호: NN-NN-NNNNNN-NN, or a region name and NN-NNNNNN-NN ("서울 12-345678-90").
fn kr_driver(s: &[char], i: usize) -> Option<usize> {
    const REGIONS: [&str; 17] =
        ["서울", "부산", "경기", "강원", "충북", "충남", "전북", "전남", "경북", "경남", "제주", "대구", "인천", "광주", "대전", "울산", "세종"];
    if let Some(n) = REGIONS.iter().find_map(|r| {
        let n = r.chars().count();
        (s.get(i..i + n)?.iter().copied().eq(r.chars()) && !i.checked_sub(1).and_then(|p| s.get(p)).is_some_and(|c| c.is_alphanumeric()))
            .then_some(n)
    }) {
        let k = i + n + usize::from(s.get(i + n) == Some(&' '));
        group(s, k, 2)?;
        group(s, k + 3, 6)?;
        group(s, k + 10, 2)?;
        return (dash(s.get(k + 2)) && dash(s.get(k + 9)) && number_ends(s, k + 12)).then_some(k + 12 - i);
    }
    if !number_starts(s, i) {
        return None;
    }
    group(s, i, 2)?;
    group(s, i + 3, 2)?;
    group(s, i + 6, 6)?;
    group(s, i + 13, 2)?;
    (dash(s.get(i + 2)) && dash(s.get(i + 5)) && dash(s.get(i + 12)) && number_ends(s, i + 15)).then_some(15)
}

/// 계좌번호: 3 or 4 groups of digits joined by `-`, 10 to 14 digits in all, the first group 3 to 6
/// digits (국민 123456-12-123456, 신한 110-123-456789, 농협 302-1234-5678-91, 카카오뱅크
/// 3333-01-1234567 …). Numbers another Korean pattern or a date already explains are left to it.
fn kr_account(s: &[char], i: usize) -> Option<usize> {
    if !number_starts(s, i) || !digit(s.get(i)) {
        return None;
    }
    let mut groups = Vec::new();
    let mut k = i;
    loop {
        let n = s.get(k..).unwrap_or_default().iter().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 {
            return None;
        }
        groups.push(n);
        k += n;
        if dash(s.get(k)) && digit(s.get(k + 1)) && groups.len() < 5 {
            k += 1;
        } else {
            break;
        }
    }
    let total: usize = groups.iter().sum();
    let first = *groups.first()?;
    if !(3..=4).contains(&groups.len()) || !(10..=14).contains(&total) || !(3..=6).contains(&first) || groups.iter().any(|g| !(2..=7).contains(g)) {
        return None;
    }
    let len = k - i;
    let explained = [kr_resident, kr_phone, kr_business, kr_driver, date].iter().any(|f| f(s, i) == Some(len));
    (!explained && number_ends(s, k)).then_some(len)
}

/// 여권번호: a type letter (M, S, R, O, D, G) and 8 digits (M12345678), or the 2021 format with a
/// letter in fifth place (M123A4567).
fn kr_passport(s: &[char], i: usize) -> Option<usize> {
    if !matches!(s.get(i), Some('M' | 'S' | 'R' | 'O' | 'D' | 'G')) || i.checked_sub(1).and_then(|p| s.get(p)).is_some_and(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let old = group(s, i + 1, 8).is_some();
    let new = group(s, i + 1, 3).is_some() && s.get(i + 4).is_some_and(char::is_ascii_uppercase) && group(s, i + 5, 4).is_some();
    ((old || new) && !s.get(i + 9).is_some_and(|c| c.is_ascii_alphanumeric())).then_some(9)
}

/// Every Korean pattern plus email addresses and credit cards, overlapping matches merged.
fn kr_all(text: &[char]) -> Vec<Range<usize>> {
    let mut all: Vec<Range<usize>> = [
        Pattern::KrResident,
        Pattern::KrPhone,
        Pattern::KrBusiness,
        Pattern::KrAccount,
        Pattern::KrPassport,
        Pattern::KrDriver,
        Pattern::Email,
        Pattern::CreditCard,
    ]
    .into_iter()
    .flat_map(|p| find(p, text))
    .collect();
    all.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
    let mut out: Vec<Range<usize>> = Vec::with_capacity(all.len());
    for r in all {
        match out.last_mut() {
            Some(last) if r.start < last.end => last.end = last.end.max(r.end),
            _ => out.push(r),
        }
    }
    out
}

/// Character ranges of `text` matching `pattern`.
pub fn find(pattern: Pattern, text: &[char]) -> Vec<Range<usize>> {
    let f: fn(&[char], usize) -> Option<usize> = match pattern {
        Pattern::KrAll => return kr_all(text),
        Pattern::KrResident => kr_resident,
        Pattern::KrPhone => kr_phone,
        Pattern::KrBusiness => kr_business,
        Pattern::KrAccount => kr_account,
        Pattern::KrPassport => kr_passport,
        Pattern::KrDriver => kr_driver,
        Pattern::Phone => phone,
        Pattern::Email => email,
        Pattern::CreditCard => credit_card,
        Pattern::Ssn => ssn,
        Pattern::Date => date,
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        match f(text, i) {
            Some(n) if n > 0 => {
                out.push(i..i + n);
                i += n;
            }
            _ => i += 1,
        }
    }
    out
}

/// The most entries a word list keeps (Find Text ▸ Multiple words or phrases).
pub const MAX_WORDS: usize = 1000;
/// The most characters an entry keeps.
pub const MAX_WORD_CHARS: usize = 256;

/// A word list from text with one word or phrase per line: trimmed, without empty lines or
/// repeats, in order, at most [`MAX_WORDS`] entries of at most [`MAX_WORD_CHARS`] characters.
pub fn word_list(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .map(|l| l.trim().chars().take(MAX_WORD_CHARS).collect::<String>())
        .filter(|w| !w.is_empty() && seen.insert(w.clone()))
        .take(MAX_WORDS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_lists_are_trimmed_unique_and_bounded() {
        assert_eq!(word_list("  John Smith \n\n\tACME\r\nJohn Smith\n  \nő ű"), ["John Smith", "ACME", "ő ű"]);
        let long = "é".repeat(MAX_WORD_CHARS + 10);
        assert_eq!(word_list(&long)[0].chars().count(), MAX_WORD_CHARS);
        let many: String = (0..MAX_WORDS + 50).map(|i| format!("w{i}\n")).collect();
        let list = word_list(&many);
        assert_eq!((list.len(), list.last().map(String::as_str)), (MAX_WORDS, Some("w999")));
        assert!(word_list("").is_empty());
    }

    fn found(p: Pattern, s: &str) -> Vec<String> {
        let c: Vec<char> = s.chars().collect();
        find(p, &c).into_iter().map(|r| c[r].iter().collect()).collect()
    }

    #[test]
    fn phone_numbers() {
        assert_eq!(
            found(Pattern::Phone, "Call (555) 123-4567 or 555.987.6543, +1 555 222 3333; not 12345 or 5551234567890"),
            ["(555) 123-4567", "555.987.6543", "+1 555 222 3333"]
        );
        // #525: international numbers match whole; "+49" used to stay visible.
        assert_eq!(found(Pattern::Phone, "Tel. +49 2151 123456."), ["+49 2151 123456"]);
        assert_eq!(
            found(Pattern::Phone, "+44 20 7946 0018, +33 1 23 45 67 89, +49 (0) 2151 123456, (0211) 123456, 030 12345678, 01 23 45 67 89"),
            ["+44 20 7946 0018", "+33 1 23 45 67 89", "+49 (0) 2151 123456", "(0211) 123456", "030 12345678", "01 23 45 67 89"]
        );
        // Not phone numbers: day-first dates, decimals, unseparated, too short or too long.
        assert_eq!(
            found(
                Pattern::Phone,
                "on 01.10.2026 or 09-10-2026, ratio 0.1234567, id 0211123456, +4921511234567, +1 234 567, 0211 12, +49 1234 5678 9012 3456"
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_tail_of_a_longer_number_is_not_a_phone_number() {
        // #1884: the last groups of a French social security number, an IBAN and a SIRET used
        // to be marked as phone numbers (and the start left visible). Example numbers.
        for s in ["NIR 1 85 05 78 006 084 36 end", "IBAN FR76 3000 6000 0112 3456 7890 189", "SIRET 123 456 782 00010"] {
            assert_eq!(found(Pattern::Phone, s), Vec::<String>::new(), "{s}");
        }
        assert_eq!(found(Pattern::Phone, "tel 01 23 45 67 89 / +33 1 23 45 67 89"), ["01 23 45 67 89", "+33 1 23 45 67 89"]);
        // Nor the head of a run too long to be one: 28 characters in, this one still goes on.
        assert_eq!(found(Pattern::Phone, "01 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7"), Vec::<String>::new());
    }

    #[test]
    fn emails() {
        assert_eq!(
            found(Pattern::Email, "Write to ada.lovelace+pdf@example.co.uk, or bob@host (no TLD) or x@y.z."),
            ["ada.lovelace+pdf@example.co.uk"]
        );
    }

    #[test]
    fn internationalized_emails() {
        assert_eq!(
            found(Pattern::Email, "Yazın: ayşe.yılmaz@örnek.com.tr, ÇAĞLAR@ŞİRKET.COM.TR; müller@bücher.de, иван@пример.рф, josé@correo.es."),
            ["ayşe.yılmaz@örnek.com.tr", "ÇAĞLAR@ŞİRKET.COM.TR", "müller@bücher.de", "иван@пример.рф", "josé@correo.es"]
        );
        // Text in scripts without spaces stays out of the address around it.
        assert_eq!(found(Pattern::Email, "メールはtaro@example.jpまで、联系support@example.cn谢谢"), ["taro@example.jp", "support@example.cn"]);
        // Still no address without a dot in the domain or with a one-letter or numeric TLD.
        assert_eq!(found(Pattern::Email, "ayşe@örnek, ş@ğ.ü, kişi@alan.123, a@b.c"), Vec::<String>::new());
    }

    #[test]
    fn credit_cards_need_a_valid_checksum() {
        assert_eq!(
            found(Pattern::CreditCard, "Visa 4111 1111 1111 1111, bad 4111 1111 1111 1112, amex 3782-822463-10005"),
            ["4111 1111 1111 1111", "3782-822463-10005"]
        );
    }

    #[test]
    fn social_security_numbers() {
        assert_eq!(
            found(Pattern::Ssn, "SSN 123-45-6789 and 123 45 6789; invalid 000-12-3456, 666-12-3456, 912-12-3456, 1234-56-7890"),
            ["123-45-6789", "123 45 6789"]
        );
    }

    #[test]
    fn dates() {
        assert_eq!(
            found(Pattern::Date, "Due 10/01/2026, 2026-10-01, 1.10.26, March 5, 2024, Jan. 7 2025 and 5 June 2023; not 10/2026 or Mayday 12"),
            ["10/01/2026", "2026-10-01", "1.10.26", "March 5, 2024", "Jan. 7 2025", "5 June 2023"]
        );
    }

    #[test]
    fn dates_with_turkish_month_names_in_any_case() {
        assert_eq!(
            found(Pattern::Date, "Tarih: 10 Ekim 2026, 3 Mayıs 2025, 5 Şubat 2024, 5 ŞUBAT 2024, 3 MAYIS 2025, 1 EKİM 2026 ve 9 kasım 2023."),
            ["10 Ekim 2026", "3 Mayıs 2025", "5 Şubat 2024", "5 ŞUBAT 2024", "3 MAYIS 2025", "1 EKİM 2026", "9 kasım 2023"]
        );
        assert_eq!(
            found(Pattern::Date, "12 Oca 2024, 7 Şub. 2025, 30 Ağu 2023, 1 ARA. 2022, 2 Haz 2021"),
            ["12 Oca 2024", "7 Şub. 2025", "30 Ağu 2023", "1 ARA. 2022", "2 Haz 2021"]
        );
        // Words that only start like a month, or a month without a day and year, stay.
        assert_eq!(
            found(Pattern::Date, "5 Martı 2024, Ekimde 2026, 3 Mayısta 2025, ara 2024, Kasım ayında, 12 Aralık, Aralık 2024"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn dates_with_german_french_and_italian_month_names() {
        assert_eq!(
            found(Pattern::Date, "am 10. Oktober 2026, 3 März 2025, le 14 juillet 2024, 1 août 2023, il 25 dicembre 2022"),
            ["10. Oktober 2026", "3 März 2025", "14 juillet 2024", "1 août 2023", "25 dicembre 2022"]
        );
        // "Mai" alone, a dotted day without a month and an unknown word are not dates.
        assert_eq!(found(Pattern::Date, "Mai 2024, Kapitel 5. Absatz 2024, 5 Oktoberfest 2024"), Vec::<String>::new());
    }

    #[test]
    fn case_folding_keeps_one_character_per_character() {
        let s: Vec<char> = "\u{130}\u{131}I\u{15E}".chars().collect();
        assert_eq!(s.iter().copied().map(fold).collect::<String>(), "iii\u{15F}");
    }

    // ---- Korean patterns (KnuPdf). Every number here is made up for the test. ----

    #[test]
    fn korean_resident_and_foreigner_registration_numbers() {
        assert_eq!(
            found(Pattern::KrResident, "주민등록번호: 900101-1234567, 외국인 850315-5123456, 공백 900101 2234567, 붙임 9001014234567"),
            ["900101-1234567", "850315-5123456", "900101 2234567", "9001014234567"]
        );
        // Masked forms still identify the person.
        assert_eq!(found(Pattern::KrResident, "신청인 900101-1******, 851224-2●●●●●●"), ["900101-1******", "851224-2●●●●●●"]);
        // Impossible month or day, gender digit 0 or 9, too long or too short, masked without a dash.
        assert_eq!(
            found(Pattern::KrResident, "901301-1234567, 900132-1234567, 900101-9234567, 900101-12345678, 900101-123456, 9001011******, 1900101-1234567"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn korean_phone_numbers() {
        assert_eq!(
            found(
                Pattern::KrPhone,
                "휴대전화 010-1234-5678, 011 234 5678, 010.9876.5432, 01012345678, 서울 02-123-4567, 02-1234-5678, (02) 123-4567, 경기 031-123-4567, 064-123-4567, 인터넷 070-1234-5678, 안심 0505-123-4567, 대표 1588-1234, 국제 +82-10-1234-5678, +82 2 123 4567"
            ),
            [
                "010-1234-5678",
                "011 234 5678",
                "010.9876.5432",
                "01012345678",
                "02-123-4567",
                "02-1234-5678",
                "(02) 123-4567",
                "031-123-4567",
                "064-123-4567",
                "070-1234-5678",
                "0505-123-4567",
                "1588-1234",
                "+82-10-1234-5678",
                "+82 2 123 4567"
            ]
        );
        // Not phone numbers: unknown area codes, unseparated landlines, too many digits, part of a longer number.
        assert_eq!(found(Pattern::KrPhone, "012-345-6789, 0212345678, 010-1234-56789, 110-123-456789, 1234-5678, 2026-10-17"), Vec::<String>::new());
    }

    #[test]
    fn korean_business_registration_numbers_need_the_check_digit() {
        // 123-45-67891 has the right check digit; 123-45-67890 does not.
        assert_eq!(found(Pattern::KrBusiness, "사업자등록번호 123-45-67891, 잘못된 번호 123-45-67890, 1234567891"), ["123-45-67891"]);
    }

    #[test]
    fn korean_driver_licence_numbers() {
        assert_eq!(
            found(Pattern::KrDriver, "면허번호 11-12-345678-90, 서울 12-345678-90, 경기12-345678-01"),
            ["11-12-345678-90", "서울 12-345678-90", "경기12-345678-01"]
        );
        assert_eq!(found(Pattern::KrDriver, "11-12-345678-901, 서울특별시 12-345678-9"), Vec::<String>::new());
    }

    #[test]
    fn korean_passport_numbers() {
        assert_eq!(found(Pattern::KrPassport, "여권 M12345678, 신형 M123A4567, 관용 S98765432"), ["M12345678", "M123A4567", "S98765432"]);
        assert_eq!(found(Pattern::KrPassport, "AM12345678, M1234567, M123456789, m12345678, MODEL123"), Vec::<String>::new());
    }

    #[test]
    fn korean_bank_account_numbers() {
        assert_eq!(
            found(
                Pattern::KrAccount,
                "국민 123456-12-123456, 신한 110-123-456789, 우리 1002-123-456789, 농협 302-1234-5678-91, 기업 123-123456-01-012, 카카오뱅크 3333-01-1234567"
            ),
            ["123456-12-123456", "110-123-456789", "1002-123-456789", "302-1234-5678-91", "123-123456-01-012", "3333-01-1234567"]
        );
        // Left to the pattern that explains them, or not account-shaped at all.
        assert_eq!(
            found(Pattern::KrAccount, "010-1234-5678, 900101-1234567, 123-45-67891, 11-12-345678-90, 2026-10-17, 12-34-56, 1234567890123"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn all_korean_personal_information_at_once() {
        let text = "성명 홍길동, 주민등록번호 900101-1234567, 연락처 010-1234-5678, 이메일 hong@example.kr, 계좌 110-123-456789, 사업자 123-45-67891, 여권 M12345678";
        assert_eq!(
            found(Pattern::KrAll, text),
            ["900101-1234567", "010-1234-5678", "hong@example.kr", "110-123-456789", "123-45-67891", "M12345678"]
        );
    }

    #[test]
    fn korean_patterns_never_panic_on_short_or_odd_text() {
        for p in PATTERNS {
            for s in ["", "-", "0", "+82", "(", "(02", "서울", "M", "900101-", "010-", "123-45-", "1-2-3-4-5-6-7-8", "+8", "0505-"] {
                let _ = found(p, s);
            }
        }
        assert!(PATTERNS.iter().all(|p| Pattern::from_id(p.id()) == Some(*p)));
    }
}
