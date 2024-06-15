#![feature(test)]

extern crate test;

#[cfg(test)]
mod tests {
    use jsonpath_rfc9535_locations::Query;
    use test::Bencher;

    #[bench]
    fn bench_short_queries(b: &mut Bencher) {
        let queries = vec![
            "$[*]",
            "$.*.a",
            "$[0,2]",
            "$['a',1]",
            "$[1,5:7]",
            "$[1,0:3]",
            "$[1,1]",
            "$[*,1]",
            "$[*,'a']",
            "$[*,0:2]",
            "$[*,*]",
            "$..[1]",
            "$..a",
            "$..*",
            "$..[*]",
            "$..['a','d']",
            "$[?@.a]",
            "$[?@.a=='b']",
            "$[?@.a=='1']",
            "$[?@.a==\"b\"]",
            "$[?@.a==\"1\"]",
            "$[?@.a==1]",
            "$[?@.a==null]",
            "$[?@.a==true]",
            "$[?@.a==false]",
            "$[?@.a==@.b]",
            "$[?@.a!='b']",
            "$[?@.a!='1']",
            "$[?@.a!=\"b\"]",
            "$[?@.a!=\"1\"]",
            "$[?@.a!=1]",
            "$[?@.a!=null]",
            "$[?@.a!=true]",
            "$[?@.a!=false]",
            "$[?@.a<'c']",
            "$[?@.a<\"c\"]",
            "$[?@.a<10]",
            "$[?@.a<null]",
            "$[?@.a<true]",
            "$[?@.a<false]",
            "$[?@.a<='c']",
            "$[?@.a<=\"c\"]",
            "$[?@.a<=10]",
            "$[?@.a<=null]",
            "$[?@.a<=true]",
            "$[?@.a<=false]",
            "$[?@.a>'c']",
            "$[?@.a>\"c\"]",
            "$[?@.a>10]",
            "$[?@.a>null]",
            "$[?@.a>true]",
            "$[?@.a>false]",
            "$[?@.a>='c']",
            "$[?@.a>=\"c\"]",
            "$[?@.a>=10]",
            "$[?@.a>=null]",
            "$[?@.a>=true]",
            "$[?@.a>=false]",
            "$[?@.a&&@.a!=null]",
            "$[?@.a&&@.b]",
            "$[?@.a||@.b]",
            "$[?@.a>0&&@.a<10]",
            "$[?@.a=='b'||@.a=='d']",
            "$[?!(@.a=='b')]",
            "$[?!@.a]",
            "$[?@[?@>1]]",
            "$[?@.a,?@.b]",
            "$[?@.a=='b',?@.b=='x']",
            "$[?@.a,?@.d]",
            "$[?@.a,1]",
            "$[?@.a,*]",
            "$[?@.a,1:]",
            "$[1, ?@.a=='b', 1:]",
            "$[?@.a==-0]",
            "$[?@.a==1.0]",
            "$[?@.a==1e2]",
            "$[?@.a==1e+2]",
            "$[?@.a==1e-2]",
            "$[?@.a==1.1]",
            "$[?@.a==1.1e2]",
            "$[?@.a==1.1e+2]",
            "$[?@.a==1.1e-2]",
            "$.values[?length(@.a) == value($..c)]",
            "$[?@<3]",
            "$[?@.a || @.b && @.b]",
            "$[?@.b && @.b || @.a]",
            "$[?(@.a || @.b) && @.a]",
            "$[?@.a && (@.b || @.a)]",
            "$[?(@.a || @.b) && @.b]",
            "$[0]",
            "$[1]",
            "$[2]",
            "$[-1]",
            "$[-2]",
            "$[-3]",
            "$[\"a\"]",
            "$[\"c\"]",
            "$[\" \"]",
            "$[\"\\\"\"]",
            "$[\"\\\\\"]",
            "$[?count(\n@.*)==1]",
            "$[?count(\t@.*)==1]",
            "$[?count(\r@.*)==1]",
            "$[?search(@ ,'[a-z]+')]",
            "$[?search(@\n,'[a-z]+')]",
            "$[?search(@\t,'[a-z]+')]",
            "$[?search(@\r,'[a-z]+')]",
            "$[?search(@, '[a-z]+')]",
            "$[?search(@,\n'[a-z]+')]",
            "$[?search(@,\t'[a-z]+')]",
            "$[?search(@,\r'[a-z]+')]",
            "$[?count(@.* )==1]",
            "$[?count(@.*\n)==1]",
            "$[?count(@.*\t)==1]",
            "$[?count(@.*\r)==1]",
            "$[?length(@ .a .b) == 3]",
            "$[?length(@\n.a\n.b) == 3]",
            "$[?length(@\t.a\t.b) == 3]",
            "$[?length(@\r.a\r.b) == 3]",
            "$..[?length(@)==length($ [0] .a)]",
            "$..[?length(@)==length($\n[0]\n.a)]",
            "$..[?length(@)==length($\t[0]\t.a)]",
            "$..[?length(@)==length($\r[0]\r.a)]",
        ];

        b.iter(|| {
            for q in queries.iter() {
                Query::standard(q).unwrap();
            }
        });
    }
}
