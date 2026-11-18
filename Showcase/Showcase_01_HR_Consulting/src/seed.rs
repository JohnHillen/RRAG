use super::db::EmployeeRecord;

/// Deterministically generates `count` realistic employee records (e.g. 300 or 3,000) for the CRM/HR database:
/// - **10% of `count` are 100% Full Matches (`Rust + SQL + >= B2`)**
/// - **5% of `count` are 1-Skill Gap Near-Matches**
/// - **85% of `count` are diverse non-matching enterprise employees**
pub fn generate_demo_employees(count: usize) -> Vec<EmployeeRecord> {
    let mut list: Vec<EmployeeRecord> = Vec::with_capacity(count);
    let target_full_matches = (count / 10).max(4);
    let target_near_matches = (count / 20).max(5);

    // 1. Canonical Showcase Profiles & Curated Multi-Hop Matches / Near-Matches (4 Full + 5 Near)
    let curated = vec![
        EmployeeRecord {
            emp_id: "EMP-1042".into(),
            name: "Thomas Schmidt".into(),
            role: "Senior Systems & Cloud Engineer".into(),
            department: "Core Platform Engineering".into(),
            skills: vec![
                "Rust".into(),
                "SQL".into(),
                "PostgreSQL".into(),
                "SurrealDB".into(),
                "Kubernetes".into(),
                "Docker".into(),
                "Terraform".into(),
                "Kafka".into(),
            ],
            english_level: "C1".into(),
            certifications: vec![
                "Cambridge Advanced C1".into(),
                "ISO/IEC 27001 Foundation".into(),
            ],
            current_project: "Projekt Alpha (Lead Backend Architekt)".into(),
            availability_percent: 40,
            hourly_rate: 145.0,
            location: "München".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1105".into(),
            name: "Michael Kurz".into(),
            role: "Senior Data & Database Architect".into(),
            department: "Data Science & Analytics".into(),
            skills: vec![
                "SQL".into(),
                "PostgreSQL".into(),
                "SurrealDB".into(),
                "Snowflake".into(),
                "BigQuery".into(),
                "Python".into(),
                "Java".into(),
                "Go".into(),
                "dbt".into(),
            ],
            english_level: "B2".into(),
            certifications: vec![
                "TOEFL iBT 95 (B2)".into(),
                "ISO/IEC 27001 Foundation".into(),
            ],
            current_project: "Projekt Alpha (Data Layer) & Projekt Titan".into(),
            availability_percent: 80,
            hourly_rate: 125.0,
            location: "Stuttgart".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1088".into(),
            name: "Sarah Weber".into(),
            role: "Systems Developer".into(),
            department: "Embedded & Performance Systems".into(),
            skills: vec![
                "Rust".into(),
                "SQL".into(),
                "SQLite".into(),
                "C++".into(),
                "Python".into(),
                "Redis".into(),
                "MongoDB".into(),
            ],
            english_level: "B1".into(),
            certifications: vec!["BSI IT-Grundschutz".into()],
            current_project: "Projekt Titan (High-Throughput Analytics Pipeline)".into(),
            availability_percent: 70,
            hourly_rate: 120.0,
            location: "Frankfurt".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1019".into(),
            name: "Dr. Elena Rostova".into(),
            role: "Principal Rust & Database Engineer".into(),
            department: "Cloud & Core Systems".into(),
            skills: vec![
                "Rust".into(),
                "SQL".into(),
                "PostgreSQL".into(),
                "C++".into(),
                "Kubernetes".into(),
                "Linux".into(),
            ],
            english_level: "C1".into(),
            certifications: vec!["ISO/IEC 27001 Lead Implementer".into()],
            current_project: "Projekt Alpha (High-Throughput Storage)".into(),
            availability_percent: 100,
            hourly_rate: 155.0,
            location: "München".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1174".into(),
            name: "Lukas Bergmann".into(),
            role: "Senior Backend Engineer (Automotive)".into(),
            department: "Automotive & Embedded Systems".into(),
            skills: vec![
                "Rust".into(),
                "SQL".into(),
                "Docker".into(),
                "Kafka".into(),
                "Linux".into(),
            ],
            english_level: "B2".into(),
            certifications: vec!["AWS Certified Developer".into()],
            current_project: "Connected Car Telemetry".into(),
            availability_percent: 90,
            hourly_rate: 130.0,
            location: "München".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1231".into(),
            name: "Hannah Lindner".into(),
            role: "Lead Platform & Systems Engineer".into(),
            department: "Cloud & Core Systems".into(),
            skills: vec![
                "Rust".into(),
                "SQL".into(),
                "SurrealDB".into(),
                "Terraform".into(),
                "Kubernetes".into(),
            ],
            english_level: "C2".into(),
            certifications: vec![
                "Certified Kubernetes Administrator (CKA)".into(),
                "ISO/IEC 27001 Foundation".into(),
            ],
            current_project: "Frei / Bench".into(),
            availability_percent: 100,
            hourly_rate: 148.0,
            location: "Berlin".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1140".into(),
            name: "Markus Vogl".into(),
            role: "Embedded & Systems Rust Developer".into(),
            department: "Automotive & Embedded Systems".into(),
            skills: vec!["Rust".into(), "SQL".into(), "C++".into(), "Linux".into()],
            english_level: "B1".into(),
            certifications: vec!["ISTQB Certified Tester".into()],
            current_project: "ECU Firmware Gateway".into(),
            availability_percent: 100,
            hourly_rate: 120.0,
            location: "Ingolstadt".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1198".into(),
            name: "Julia Hartmann".into(),
            role: "Systems & Cloud Engineer".into(),
            department: "Cloud & Core Systems".into(),
            skills: vec![
                "Rust".into(),
                "Kubernetes".into(),
                "Docker".into(),
                "Terraform".into(),
                "NoSQL".into(),
            ],
            english_level: "C1".into(),
            certifications: vec!["Certified Kubernetes Security Specialist (CKS)".into()],
            current_project: "Cloud Native Mesh".into(),
            availability_percent: 80,
            hourly_rate: 138.0,
            location: "Hamburg".into(),
        },
        EmployeeRecord {
            emp_id: "EMP-1265".into(),
            name: "David Chen".into(),
            role: "Senior Data & Analytics Architect".into(),
            department: "Data & Analytics".into(),
            skills: vec![
                "SQL".into(),
                "PostgreSQL".into(),
                "Python".into(),
                "Snowflake".into(),
                "Kafka".into(),
            ],
            english_level: "C1".into(),
            certifications: vec!["Snowflake SnowPro Core".into()],
            current_project: "Enterprise DWH Modernization".into(),
            availability_percent: 100,
            hourly_rate: 140.0,
            location: "München".into(),
        },
    ];

    for emp in curated {
        if list.len() < count {
            list.push(emp);
        }
    }

    let mut current_full_matches = 4usize;
    let mut current_near_matches = 5usize;

    let first_names = [
        "Alexander", "Laura", "Maximilian", "Sophie", "Felix", "Marie", "Paul", "Katharina",
        "Leon", "Anna", "Lukas", "Lena", "Jonas", "Lea", "Tim", "Jannik", "Jan", "Clara",
        "Niklas", "Johanna", "Philipp", "Charlotte", "Fabian", "Victoria", "Simon", "Amelie",
        "Tobias", "Emilia", "Florian", "Miriam", "Sebastian", "Nadine", "Christian", "Melanie",
        "Daniel", "Sabrina", "Patrick", "Lisa", "Matthias", "Vanessa", "Andreas", "Tanja",
        "Stefan", "Kristina", "Benjamin", "Franziska", "Dominik", "Anja", "Marcel", "Steffi",
    ];
    let last_names = [
        "Müller", "Meier", "Schulz", "Wagner", "Becker", "Hoffmann", "Schäfer", "Koch",
        "Bauer", "Richter", "Klein", "Wolf", "Schröder", "Neumann", "Schwarz", "Zimmermann",
        "Braun", "Krüger", "Hofmann", "Hartmann", "Lange", "Schmitt", "Werner", "Schmitz",
        "Krause", "Maier", "Lehmann", "Schmid", "Schulze", "Maier", "Köhler", "Herrmann",
        "König", "Walter", "Mayer", "Huber", "Kaiser", "Fuchs", "Peters", "Lang",
    ];

    let full_match_templates: [(&str, &str, &[&str], &str, &[&str]); 4] = [
        (
            "Senior Rust & Distributed Systems Engineer",
            "Cloud & Core Systems",
            &["Rust", "SQL", "PostgreSQL", "Kubernetes", "Docker", "gRPC"],
            "C1",
            &["ISO/IEC 27001 Foundation", "CKA"],
        ),
        (
            "Backend & Database Platform Engineer",
            "Core Platform Engineering",
            &["Rust", "SQL", "SurrealDB", "Kafka", "Linux", "Terraform"],
            "B2",
            &["AWS Certified Solutions Architect"],
        ),
        (
            "Automotive Cloud & Telemetry Architect",
            "Automotive & Embedded Systems",
            &["Rust", "SQL", "PostgreSQL", "C++", "Docker", "Kafka"],
            "C1",
            &["BSI IT-Grundschutz", "ISO/IEC 27001 Foundation"],
        ),
        (
            "Staff Data & Storage Systems Engineer",
            "Data & Analytics",
            &["Rust", "SQL", "SurrealDB", "PostgreSQL", "Python", "Kubernetes"],
            "C2",
            &["Google Cloud Professional Data Engineer"],
        ),
    ];

    let near_match_templates: [(&str, &str, &[&str], &str, &[&str]); 3] = [
        (
            "Embedded Rust & Database Developer",
            "Automotive & Embedded Systems",
            &["Rust", "SQL", "PostgreSQL", "C++", "Linux"],
            "B1",
            &["BSI IT-Grundschutz"],
        ),
        (
            "Senior SQL & Cloud Data Engineer",
            "Data & Analytics",
            &["SQL", "PostgreSQL", "Snowflake", "Python", "Kafka"],
            "C1",
            &["ISO/IEC 27001 Foundation"],
        ),
        (
            "Cloud Native Rust Microservices Engineer",
            "Cloud & Core Systems",
            &["Rust", "NoSQL", "MongoDB", "Redis", "Kubernetes", "Docker"],
            "B2",
            &["CKA"],
        ),
    ];

    let role_templates: [(&str, &str, &[&str], &str, &[&str]); 10] = [
        (
            "Frontend & UI Engineer",
            "Digital Experience",
            &["TypeScript", "React", "HTML/CSS", "GraphQL", "Cypress"],
            "B2",
            &["W3C Accessibility Specialist"],
        ),
        (
            "SAP S/4HANA Consultant",
            "Enterprise ERP",
            &["SAP", "ABAP", "Fiori", "HANA", "BTP"],
            "B2",
            &["SAP Certified Application Associate"],
        ),
        (
            "Java Enterprise Architect",
            "Enterprise Backend",
            &["Java", "Spring Boot", "Kubernetes", "Docker", "Kafka"],
            "C1",
            &["Oracle Certified Professional Java SE"],
        ),
        (
            "Machine Learning & AI Engineer",
            "AI & Data Science",
            &["Python", "PyTorch", "TensorFlow", "Docker", "MLOps"],
            "C1",
            &["TensorFlow Developer Certificate"],
        ),
        (
            "DevOps & Site Reliability Engineer",
            "Cloud & Infrastructure",
            &["Terraform", "Kubernetes", "Docker", "Ansible", "GitLab CI"],
            "B2",
            &["AWS Certified DevOps Engineer"],
        ),
        (
            "Cybersecurity & GRC Consultant",
            "Security & Compliance",
            &["ISO/IEC 27001", "BSI IT-Grundschutz", "SIEM", "Zero Trust", "Linux"],
            "C1",
            &["CISSP", "ISO/IEC 27001 Lead Auditor"],
        ),
        (
            "Agile Scrum Master & Product Owner",
            "Product & Delivery",
            &["Scrum", "Kanban", "Jira", "Confluence", "SAFe"],
            "C1",
            &["Professional Scrum Master (PSM II)"],
        ),
        (
            "iOS & Android Mobile Developer",
            "Mobile Engineering",
            &["Swift", "Kotlin", "Flutter", "Firebase", "REST"],
            "B2",
            &["Google Associate Android Developer"],
        ),
        (
            "BI & Reporting Specialist",
            "Data & Analytics",
            &["PowerBI", "Tableau", "SQL", "Excel", "DAX"],
            "B1",
            &["Microsoft Certified: Power BI Data Analyst"],
        ),
        (
            "C# / .NET Backend Developer",
            "Enterprise Backend",
            &["C#", ".NET Core", "Azure", "Docker", "Microservices"],
            "B2",
            &["Microsoft Certified: Azure Developer Associate"],
        ),
    ];

    let locations = [
        "München", "Stuttgart", "Frankfurt", "Berlin", "Hamburg", "Köln", "Düsseldorf", "Nürnberg",
    ];
    let active_projects = [
        "Portal Relaunch 2026",
        "SAP S/4HANA Rollout",
        "Cloud Migration Wave 2",
        "Customer Mobile App",
        "Zero-Trust Security Audit",
        "Internal AI Platform",
    ];

    let mut idx = 1usize;
    while list.len() < count {
        let emp_num = 1000 + idx;
        let emp_id = format!("EMP-{:04}", emp_num);
        if list.iter().any(|e| e.emp_id == emp_id) {
            idx += 1;
            continue;
        }

        let first_name = first_names[(idx * 7 + idx / 40) % first_names.len()];
        let last_name = last_names[(idx * 13 + idx / 30) % last_names.len()];
        let name = format!("{} {}", first_name, last_name);

        let (role, dept, skills_slice, eng, certs_slice) =
            if idx % 10 == 0 && current_full_matches < target_full_matches {
                current_full_matches += 1;
                full_match_templates[current_full_matches % full_match_templates.len()]
            } else if idx % 20 == 5 && current_near_matches < target_near_matches {
                current_near_matches += 1;
                near_match_templates[current_near_matches % near_match_templates.len()]
            } else if (count - list.len())
                <= (target_full_matches.saturating_sub(current_full_matches))
            {
                current_full_matches += 1;
                full_match_templates[current_full_matches % full_match_templates.len()]
            } else if (count - list.len())
                <= (target_full_matches.saturating_sub(current_full_matches)
                    + target_near_matches.saturating_sub(current_near_matches))
            {
                current_near_matches += 1;
                near_match_templates[current_near_matches % near_match_templates.len()]
            } else {
                role_templates[idx % role_templates.len()]
            };

        let mix = idx + (idx / 10) * 3 + (idx / 20) * 7;
        let avail: i64 = match mix % 5 {
            0 => 100,
            1 => 80,
            2 => 60,
            3 => 50,
            _ => 20,
        };
        let rate = 95.0 + ((mix * 11) % 65) as f64;
        let loc = locations[mix % locations.len()];
        let proj = if avail >= 100 {
            "Frei / Bench"
        } else {
            active_projects[mix % active_projects.len()]
        };

        list.push(EmployeeRecord {
            emp_id,
            name,
            role: role.to_string(),
            department: dept.to_string(),
            skills: skills_slice.iter().map(|s| s.to_string()).collect(),
            english_level: eng.to_string(),
            certifications: certs_slice.iter().map(|s| s.to_string()).collect(),
            current_project: proj.to_string(),
            availability_percent: avail,
            hourly_rate: rate,
            location: loc.to_string(),
        });

        idx += 1;
    }

    list
}
