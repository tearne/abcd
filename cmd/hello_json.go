package main

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"path"
	"os"
	"github.com/mitchellh/mapstructure"
)

type Parameter struct {
	alpha float64
	beta  float64
}

type Particle struct {
	S []float64 `json:"s"`
	W float64 `json:"w"`
	P []Parameter `json:"p"`
}

type Test struct {
	Value int
	Cake string
	Array []int
	Float float64
}

func load(filename string) string {
	pwd, _ := os.Getwd()
	bytes, err := ioutil.ReadFile(path.Join(pwd,filename))
	if err != nil {
		fmt.Println(err)
	}
	return string(bytes)
}

func main() {

	//https://blog.golang.org/json
	json_str := load("/examples/particle.json")
	fmt.Println(json_str)
	var temp interface{}
	err := json.Unmarshal([]byte(json_str), &temp)
	if err != nil {
		fmt.Println(err)
	}
	// fmt.Printf("Result is: %+v\n", temp)
	// fmt.Printf(" : %+v\n", temp.(map[string]interface{})["particle"])

	var result Particle
	cfg := &mapstructure.DecoderConfig{
        Metadata: nil,
        Result:   &result,
        TagName:  "json",
    }
	decoder, _ := mapstructure.NewDecoder(cfg)
	input := temp.(map[string]interface{})["particle"]
	decoder.Decode(input)
	fmt.Printf("Result is: %+v\n", result)




	
	// json_str = load("/examples/simple.json")
	// fmt.Println(json_str)

	// var test Test
	// err = json.Unmarshal([]byte(json_str), &test)
	// if err != nil {
	// 	fmt.Println(err)
	// }
	// fmt.Printf("Result is: %+v\n", test)


}
