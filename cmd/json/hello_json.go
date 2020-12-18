package main

import (
	"encoding/json"
	"fmt"
)

type Parameter struct {
	alpha float64
	beta  float64
}

type Particle struct {
	s []float64
	w float64
	p []Parameter
}

type Test struct {
	Value int
}

func main() {
	my_json := `{
		"particle" : {
			"s" : [10,20,30],
			"w" : 0.12345,
			"p" : {
				"alpha": 0.1,
				"beta": 10.1
			}
		}
	}`

	var result Particle

	err := json.Unmarshal([]byte(my_json), &result)

	if err != nil {
		fmt.Println(err)
	}

	fmt.Printf("Result is: %+v", result)

	simple := `{"value": 2}`
	var result2 Test
	err2 := json.Unmarshal([]byte(simple), &result2)

	if err2 != nil {
		fmt.Println(err2)
	}

	fmt.Printf("Result is: %+v", result2)
}
